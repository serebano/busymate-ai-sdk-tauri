//! # Busymate AI SDK for Tauri — `tauri-plugin-busymate-ai`
//!
//! Opens the hosted Busymate chat in its own webview window and connects it to
//! your app through two optional, independently versioned bridges:
//!
//! - **Identity** — `busymate.bridge/2` (wire v2, immutable build 2.0.0). Your
//!   app's Rust side answers `account` and `mint`; the page never sees a raw
//!   account id, a signing key or anything but your own mint, an https open and
//!   a fixed action set.
//! - **Microphone** — `BusymateMicrophone` (wire 1, adapter 1.0.0). The OS is
//!   asked only when the user taps the chat's microphone or voice control.
//!
//! Only the chat window's exact allowed https origins can reach the two bridge
//! commands: the plugin adds a runtime capability scoped to those origins and
//! that window when the chat opens, and checks the origin again in Rust.
//!
//! ```rust,no_run
//! use tauri_plugin_busymate_ai::{BusymateAiExt, ChatOptions, MintToken};
//!
//! tauri::Builder::default()
//!     .plugin(
//!         tauri_plugin_busymate_ai::Builder::new()
//!             .account(|_app| Some("user-42".to_string()))
//!             .mint(|_app, request| async move {
//!                 // Call YOUR authenticated backend with request.nonce, request.aud,
//!                 // request.assistant and request.origin; never sign in the app.
//!                 Ok(Some(MintToken { token: "…".into(), nonce: request.nonce }))
//!             })
//!             .build(),
//!     )
//!     // Required for the microphone: decides the webview's own media request.
//!     .on_permission_request(tauri_plugin_busymate_ai::permission_handler)
//!     .setup(|app| {
//!         app.busymate_ai().open_chat(ChatOptions {
//!             url: "https://acme.busymate.ai/".into(),
//!             assistant: Some("acme".into()),
//!             ..Default::default()
//!         })?;
//!         Ok(())
//!     });
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::ipc::CapabilityBuilder;
use tauri::plugin::{Builder as PluginBuilder, TauriPlugin};
use tauri::webview::{NewWindowResponse, PageLoadEvent, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Emitter, EventTarget, Manager, Runtime, Url, Webview, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub mod bridge;
mod commands;
pub mod microphone;
pub mod origin;
pub mod os_permission;
mod platform;

pub use bridge::{AppInfo, BUILD as IDENTITY_BUILD, VERSION as IDENTITY_WIRE};
pub use microphone::{ADAPTER as MICROPHONE_ADAPTER, WIRE as MICROPHONE_WIRE};

/// This SDK's distribution version.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The default guest chat. `channel=embed` is the hosted product's channel for
/// desktop shells; `desktop` is reserved for Busymate's own Console app.
pub const DEFAULT_CHAT_URL: &str = "https://busymate.ai/support/busyproxy?channel=embed&locale=en";
/// The event your local frontend receives (see [`ChatEvent`]).
pub const CHAT_EVENT: &str = "busymate-ai://chat";

const CHAT_INIT: &str = include_str!("chat-init.js");
const LABEL_PREFIX: &str = "busymate-chat-";

// ── public types ────────────────────────────────────────────────────────────

/// What the chat passes to your mint, plus `assistant` and `origin`, which the
/// bridge sets itself and a page can never choose. Unknown future fields are in `raw`.
#[derive(Debug, Clone, Serialize)]
pub struct MintRequest {
    pub nonce: String,
    pub aud: Option<String>,
    pub reason: Option<String>,
    pub assistant: String,
    pub origin: String,
    pub raw: Map<String, Value>,
}

impl MintRequest {
    fn from_raw(raw: Map<String, Value>) -> Self {
        let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_string);
        Self {
            nonce: text("nonce").unwrap_or_default(),
            aud: text("aud"),
            reason: text("reason"),
            assistant: text("assistant").unwrap_or_default(),
            origin: text("origin").unwrap_or_default(),
            raw,
        }
    }
}

/// A fresh, single-use launch assertion from YOUR backend's signer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintToken {
    pub token: String,
    pub nonce: String,
}

/// Why a mint did not produce a token. `NotSignedIn` keeps the chat a guest.
#[derive(Debug, Clone)]
pub enum MintError {
    NotSignedIn,
    Failed(String),
}

pub type MintFuture = Pin<Box<dyn Future<Output = Result<Option<MintToken>, MintError>> + Send>>;

/// How to open the hosted chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatOptions {
    /// The tenant's https chat URL, query parameters included.
    pub url: String,
    /// Assistant slug: allows exactly `https://<assistant>.busymate.ai`.
    pub assistant: Option<String>,
    /// Extra EXACT https origins (for example `https://busymate.ai` or your mapped support host).
    pub origins: Vec<String>,
    /// Install the identity bridge (`window.BusymateBridge`). Needs `assistant`.
    pub identity: bool,
    /// Install the microphone adapter (`window.BusymateMicrophone`).
    pub microphone: bool,
    pub title: String,
    pub width: f64,
    pub height: f64,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            url: DEFAULT_CHAT_URL.into(),
            assistant: None,
            origins: Vec::new(),
            identity: true,
            microphone: true,
            title: "Support".into(),
            width: 420.0,
            height: 680.0,
        }
    }
}

/// A host-side notice for your app's own UI or logs. Never carries tokens,
/// nonces, account ids, envelopes or audio.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    /// The chat window label.
    pub label: String,
    /// `opened`, `closed`, `page-load`, `navigation-refused`, `external-open`,
    /// `bridge-op`, `bridge-denied`, `bridge-event`, `mint`, `action`,
    /// `close`, `microphone-request`, `media-decision`, `renderer-restored`.
    pub kind: String,
    /// A short non-secret detail (an op name, an origin, `granted`/`denied`…).
    pub detail: Option<String>,
}

// ── plugin builder ──────────────────────────────────────────────────────────

type AccountFn<R> = Arc<dyn Fn(&AppHandle<R>) -> Option<String> + Send + Sync>;
type MintFn<R> = Arc<dyn Fn(&AppHandle<R>, MintRequest) -> MintFuture + Send + Sync>;
type ActionFn<R> = Arc<dyn Fn(&AppHandle<R>, &str, &str, &Map<String, Value>) -> bool + Send + Sync>;
type CloseFn<R> = Arc<dyn Fn(&AppHandle<R>, &str) + Send + Sync>;
type OpenFn<R> = Arc<dyn Fn(&AppHandle<R>, &str, &str) -> bool + Send + Sync>;
type EventFn<R> = Arc<dyn Fn(&AppHandle<R>, &ChatEvent) + Send + Sync>;

struct Callbacks<R: Runtime> {
    account: Option<AccountFn<R>>,
    mint: Option<MintFn<R>>,
    on_action: Option<ActionFn<R>>,
    on_close: Option<CloseFn<R>>,
    open: Option<OpenFn<R>>,
    on_event: Option<EventFn<R>>,
    app: Option<AppInfo>,
}

/// Configure the plugin. Every callback is optional; without `account`/`mint`
/// the identity bridge answers `not_signed_in` and the chat stays a guest.
pub struct Builder<R: Runtime> {
    callbacks: Callbacks<R>,
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runtime> Builder<R> {
    pub fn new() -> Self {
        Self {
            callbacks: Callbacks { account: None, mint: None, on_action: None, on_close: None, open: None, on_event: None, app: None },
        }
    }

    /// The signed-in account id, or `None` when signed out. Read on every
    /// `state`/`mint`; only SHA-256(`appId:account`) reaches the chat.
    pub fn account<F>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>) -> Option<String> + Send + Sync + 'static,
    {
        self.callbacks.account = Some(Arc::new(f));
        self
    }

    /// Mint a fresh single-use launch assertion through YOUR authenticated
    /// backend. Never cache or reuse a token/nonce pair; never sign in the app.
    pub fn mint<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>, MintRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<MintToken>, MintError>> + Send + 'static,
    {
        self.callbacks.mint = Some(Arc::new(move |app, request| Box::pin(f(app, request))));
        self
    }

    /// A fixed app-owned action set (`close`, `ready`, …). Return true only when handled.
    /// Arguments: app, chat window label, action name, params.
    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>, &str, &str, &Map<String, Value>) -> bool + Send + Sync + 'static,
    {
        self.callbacks.on_action = Some(Arc::new(f));
        self
    }

    /// Host dismissal; the fallback for an unhandled `close` action.
    /// Without it, `close` is answered `handled:false`.
    pub fn on_close<F>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>, &str) + Send + Sync + 'static,
    {
        self.callbacks.on_close = Some(Arc::new(f));
        self
    }

    /// Replace how an https URL is opened outside the app (`mode` is `external`
    /// or `auth`). The default opens the system browser for both.
    pub fn open_url<F>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>, &str, &str) -> bool + Send + Sync + 'static,
    {
        self.callbacks.open = Some(Arc::new(f));
        self
    }

    /// Observe [`ChatEvent`]s in Rust (they are also emitted to your local
    /// frontend as [`CHAT_EVENT`]).
    pub fn on_event<F>(mut self, f: F) -> Self
    where
        F: Fn(&AppHandle<R>, &ChatEvent) + Send + Sync + 'static,
    {
        self.callbacks.on_event = Some(Arc::new(f));
        self
    }

    /// Override what `hello` reports as your app (defaults to the Tauri config's
    /// identifier and version). `id` also salts the account key.
    pub fn app_info(mut self, app: AppInfo) -> Self {
        self.callbacks.app = Some(app);
        self
    }

    pub fn build(self) -> TauriPlugin<R> {
        let callbacks = Mutex::new(Some(self.callbacks));
        PluginBuilder::new("busymate-ai")
            .invoke_handler(tauri::generate_handler![
                commands::open_chat,
                commands::close_chat,
                commands::account_changed,
                commands::bridge,
                commands::microphone
            ])
            .setup(move |app, _api| {
                let callbacks = callbacks.lock().unwrap().take().expect("plugin set up once");
                app.manage(BusymateAi::new(app.clone(), callbacks));
                Ok(())
            })
            .build()
    }
}

// ── runtime state ───────────────────────────────────────────────────────────

pub(crate) struct Chat {
    pub label: String,
    pub bridge: Option<bridge::BridgeCore>,
    /// Exact origins for navigation, the microphone and the runtime capability.
    pub allowed: Vec<String>,
    pub microphone: bool,
    pub pending_restored: AtomicBool,
    pub hello_seen: AtomicBool,
}

struct Inner<R: Runtime> {
    callbacks: Callbacks<R>,
    chats: Mutex<HashMap<String, Arc<Chat>>>,
    counter: AtomicU64,
    app: AppInfo,
}

/// Access with [`BusymateAiExt::busymate_ai`].
pub struct BusymateAi<R: Runtime> {
    app: AppHandle<R>,
    inner: Arc<Inner<R>>,
}

impl<R: Runtime> Clone for BusymateAi<R> {
    fn clone(&self) -> Self {
        Self { app: self.app.clone(), inner: self.inner.clone() }
    }
}

/// `app.busymate_ai()` on an `App`, `AppHandle`, window or webview.
pub trait BusymateAiExt<R: Runtime> {
    fn busymate_ai(&self) -> &BusymateAi<R>;
}

impl<R: Runtime, T: Manager<R>> BusymateAiExt<R> for T {
    fn busymate_ai(&self) -> &BusymateAi<R> {
        self.state::<BusymateAi<R>>().inner()
    }
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<tauri::Error> for Error {
    fn from(error: tauri::Error) -> Self {
        Error(error.to_string())
    }
}

/// Validate options without opening anything: the chat URL, the slug and every origin.
/// Returns the exact allowed origin list.
pub fn validate_options(options: &ChatOptions) -> Result<Vec<String>, Error> {
    let url = Url::parse(options.url.trim()).map_err(|_| Error("The chat URL is not a valid URL.".into()))?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(Error("The chat URL must use https, with no embedded credentials.".into()));
    }
    if let Some(slug) = options.assistant.as_deref() {
        if !origin::valid_assistant(slug) {
            return Err(Error("Use a valid assistant slug (a-z, 0-9 and inner hyphens).".into()));
        }
    }
    if options.identity && options.assistant.is_none() {
        return Err(Error("The identity bridge needs the assistant slug.".into()));
    }
    let extra = origin::validate_origins(&options.origins)
        .map_err(|entry| Error(format!("Not an exact https origin: {entry}. No paths, wildcards or credentials.")))?;
    let allowed = origin::allowed_origins(options.assistant.as_deref(), &extra);
    let chat_origin = origin::origin_of_url(&url).unwrap_or_default();
    if !allowed.contains(&chat_origin) {
        return Err(Error(format!("Include the chat's own origin ({chat_origin}) in the allowed origins.")));
    }
    Ok(allowed)
}

fn webview_name() -> String {
    let engine = if cfg!(target_os = "macos") {
        "WKWebView"
    } else if cfg!(windows) {
        "WebView2"
    } else {
        "WebKitGTK"
    };
    format!("Tauri/{} {engine}", tauri::VERSION)
}

impl<R: Runtime> BusymateAi<R> {
    fn new(app: AppHandle<R>, mut callbacks: Callbacks<R>) -> Self {
        let config = app.config();
        let app_info = callbacks.app.take().unwrap_or_else(|| AppInfo {
            id: config.identifier.clone(),
            version: config.version.clone(),
            build: None,
        });
        Self {
            app,
            inner: Arc::new(Inner { callbacks, chats: Mutex::new(HashMap::new()), counter: AtomicU64::new(0), app: app_info }),
        }
    }

    pub(crate) fn chat(&self, label: &str) -> Option<Arc<Chat>> {
        self.inner.chats.lock().unwrap().get(label).cloned()
    }

    /// Labels of the open chat windows.
    pub fn chat_labels(&self) -> Vec<String> {
        self.inner.chats.lock().unwrap().keys().cloned().collect()
    }

    pub(crate) fn account(&self) -> Option<String> {
        self.inner.callbacks.account.as_ref().and_then(|f| f(&self.app))
    }

    pub(crate) fn notify(&self, label: &str, kind: &str, detail: Option<String>) {
        let event = ChatEvent { label: label.to_string(), kind: kind.to_string(), detail };
        if let Some(f) = &self.inner.callbacks.on_event {
            f(&self.app, &event);
        }
        // Never to a chat window: only your own frontend hears these.
        let chats = self.chat_labels();
        let _ = self.app.emit_filter(CHAT_EVENT, event, |target| {
            let label = match target {
                EventTarget::AnyLabel { label }
                | EventTarget::Window { label }
                | EventTarget::Webview { label }
                | EventTarget::WebviewWindow { label } => label,
                EventTarget::Any | EventTarget::App => return true,
                _ => return false,
            };
            !chats.contains(label)
        });
    }

    /// Open the hosted chat in a new window. Returns the window label.
    pub fn open_chat(&self, options: ChatOptions) -> Result<String, Error> {
        let allowed = validate_options(&options)?;
        let url = Url::parse(options.url.trim()).map_err(|_| Error("The chat URL is not a valid URL.".into()))?;
        let n = self.inner.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let label = format!("{LABEL_PREFIX}{n}");

        let bridge = if options.identity {
            options.assistant.as_deref().map(|assistant| {
                let extra = origin::validate_origins(&options.origins).unwrap_or_default();
                bridge::BridgeCore::new(assistant, &extra, self.inner.app.clone(), Some(webview_name()))
            })
        } else {
            None
        };

        // The ONLY IPC the remote chat may use: two commands, this window, these exact origins.
        if bridge.is_some() || options.microphone {
            let mut capability = CapabilityBuilder::new(format!("busymate-ai-chat-{n}")).local(false).window(label.clone());
            for origin in &allowed {
                capability = capability.remote(format!("{origin}/*"));
            }
            if bridge.is_some() {
                capability = capability.permission("busymate-ai:allow-bridge");
            }
            if options.microphone {
                capability = capability.permission("busymate-ai:allow-microphone");
            }
            self.app.add_capability(capability)?;
        }

        let chat = Arc::new(Chat {
            label: label.clone(),
            bridge,
            allowed: allowed.clone(),
            microphone: options.microphone,
            pending_restored: AtomicBool::new(false),
            hello_seen: AtomicBool::new(false),
        });
        let script_config = serde_json::json!({
            "origins": allowed,
            "identity": chat.bridge.is_some(),
            "microphone": options.microphone,
        });
        let script = CHAT_INIT.replace("__BUSYMATE_CONFIG__", &script_config.to_string());

        let nav_allowed = allowed.clone();
        let nav_self = self.clone();
        let nav_label = label.clone();
        let popup_self = self.clone();
        let popup_label = label.clone();
        let load_self = self.clone();

        self.inner.chats.lock().unwrap().insert(label.clone(), chat.clone());
        let built = WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::External(url))
            .title(&options.title)
            .inner_size(options.width, options.height)
            .initialization_script(script)
            .on_navigation(move |url| {
                let ok = url.scheme() == "about"
                    || origin::origin_of_url(url).map(|o| nav_allowed.contains(&o)).unwrap_or(false);
                if !ok {
                    nav_self.notify(&nav_label, "navigation-refused", origin::origin_of_url(url));
                }
                ok
            })
            .on_new_window(move |url, _features| {
                // A popup never becomes an in-app window: https goes to the system browser.
                if url.scheme() == "https" {
                    let opened = popup_self.open_external(url.as_str(), "external");
                    popup_self.notify(&popup_label, "external-open", Some(if opened { "opened" } else { "failed" }.into()));
                }
                NewWindowResponse::Deny
            })
            .on_page_load(move |window, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    load_self.notify(window.label(), "page-load", origin::origin_of_url(payload.url()));
                }
            })
            .build();
        let window = match built {
            Ok(window) => window,
            Err(error) => {
                self.inner.chats.lock().unwrap().remove(&label);
                return Err(error.into());
            }
        };

        let events_self = self.clone();
        let events_label = label.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Focused(true) => events_self.emit_event(&events_label, "resume"),
            WindowEvent::Destroyed => {
                events_self.inner.chats.lock().unwrap().remove(&events_label);
                events_self.notify(&events_label, "closed", None);
            }
            _ => {}
        });
        platform::install_renderer_recovery(&window, self.clone());
        self.notify(&label, "opened", origin::origin_of_url(&window.url()?));
        Ok(label)
    }

    /// Close one chat window, or every chat window when `label` is `None`.
    pub fn close_chat(&self, label: Option<&str>) -> Result<(), Error> {
        let labels = match label {
            Some(label) => vec![label.to_string()],
            None => self.chat_labels(),
        };
        for label in labels {
            if self.chat(&label).is_none() {
                continue;
            }
            if let Some(window) = self.app.get_webview_window(&label) {
                window.destroy()?;
            }
        }
        Ok(())
    }

    /// Tell open chats the signed-in account changed (sign-in, sign-out, switch).
    pub fn account_changed(&self) {
        for label in self.chat_labels() {
            self.emit_event(&label, "state");
        }
    }

    /// Mark a chat for `restored` and reload it after its web content process died.
    pub fn renderer_gone(&self, label: &str) {
        if let Some(chat) = self.chat(label) {
            chat.pending_restored.store(true, Ordering::SeqCst);
            if let Some(window) = self.app.get_webview_window(label) {
                let _ = window.reload();
            }
            self.notify(label, "renderer-restored", None);
        }
    }

    pub(crate) fn open_external(&self, url: &str, mode: &str) -> bool {
        match &self.inner.callbacks.open {
            Some(f) => f(&self.app, url, mode),
            None => open::that_detached(url).is_ok(),
        }
    }

    pub(crate) fn mint_fn(&self) -> Option<MintFn<R>> {
        self.inner.callbacks.mint.clone()
    }

    pub(crate) fn handle_action(&self, label: &str, name: &str, params: &Map<String, Value>) -> bool {
        let mut handled = self.inner.callbacks.on_action.as_ref().map(|f| f(&self.app, label, name, params)).unwrap_or(false);
        if !handled && name == "close" {
            if let Some(on_close) = &self.inner.callbacks.on_close {
                on_close(&self.app, label);
                handled = true;
                self.notify(label, "close", None);
            }
        }
        handled
    }

    pub(crate) fn app_handle(&self) -> &AppHandle<R> {
        &self.app
    }

    /// Push a bridge event (`resume`, `state`, `restored`) to one chat — only
    /// after its `hello`, and only while its document is on an allowed origin.
    pub(crate) fn emit_event(&self, label: &str, ev: &str) {
        let Some(chat) = self.chat(label) else { return };
        let Some(core) = chat.bridge.as_ref() else { return };
        if !chat.hello_seen.load(Ordering::SeqCst) {
            return;
        }
        let Some(window) = self.app.get_webview_window(label) else { return };
        let Ok(url) = window.url() else { return };
        if !core.origin_allowed(origin::origin_of_url(&url).as_deref()) {
            return;
        }
        let r = if ev == "state" { bridge::state(&core.app.id, self.account().as_deref()) } else { Value::Object(Map::new()) };
        if window.eval(bridge::deliver_script(&bridge::event(ev, r))).is_ok() {
            self.notify(label, "bridge-event", Some(ev.to_string()));
        }
    }
}

/// Tauri's app-level media permission hook for the chat window. Register it:
/// `tauri::Builder::default().on_permission_request(tauri_plugin_busymate_ai::permission_handler)`.
///
/// For a chat window: the microphone is allowed only with the adapter enabled,
/// the chat document on an allowed https origin and the OS permission granted;
/// camera and screen capture are denied. Every other webview, and every other
/// permission kind, keeps the platform default.
pub fn permission_handler<R: Runtime>(webview: Webview<R>, kind: PermissionKind) -> PermissionResponse {
    let Some(state) = webview.try_state::<BusymateAi<R>>() else { return PermissionResponse::Default };
    let Some(chat) = state.chat(webview.label()) else { return PermissionResponse::Default };
    match kind {
        PermissionKind::Microphone => {
            let origin_ok = webview
                .url()
                .ok()
                .and_then(|url| origin::origin_of_url(&url))
                .map(|origin| chat.allowed.contains(&origin))
                .unwrap_or(false);
            let granted = chat.microphone && origin_ok && os_permission::status() == os_permission::Status::Granted;
            state.notify(&chat.label, "media-decision", Some(if granted { "granted" } else { "denied" }.into()));
            if granted {
                PermissionResponse::Allow
            } else {
                PermissionResponse::Deny
            }
        }
        PermissionKind::Camera | PermissionKind::DisplayCapture => {
            state.notify(&chat.label, "media-decision", Some("denied".into()));
            PermissionResponse::Deny
        }
        _ => PermissionResponse::Default,
    }
}

/// macOS: forward WebKit's web content process termination for chat windows:
/// `.on_web_content_process_terminate(|webview| tauri_plugin_busymate_ai::web_content_process_terminated(webview))`.
/// The chat reloads and receives `restored` after its next `hello`. Other
/// webviews are reloaded as Tauri does by default.
#[cfg(target_os = "macos")]
pub fn web_content_process_terminated<R: Runtime>(webview: &Webview<R>) {
    match webview.try_state::<BusymateAi<R>>() {
        Some(state) if state.chat(webview.label()).is_some() => state.renderer_gone(webview.label()),
        _ => {
            let _ = webview.reload();
        }
    }
}

// Busymate AI SDK for Tauri — example app.
//
// The settings window (../index.html) is this app's own local frontend. The
// hosted chat opens in a separate window owned by the SDK plugin. Identity
// callbacks live HERE, on the Rust side, as they would in your app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod demo;

use std::sync::Mutex;

use tauri::{Manager, State};
use tauri_plugin_busymate_ai::BusymateAiExt;

#[derive(Default)]
struct DemoState(Mutex<demo::DemoIdentity>);

impl DemoState {
    fn current(&self) -> demo::DemoIdentity {
        self.0.lock().map(|identity| identity.clone()).unwrap_or_default()
    }
}

/// Settings → Rust. Validated; held in memory only.
#[tauri::command]
fn demo_apply_identity(state: State<'_, DemoState>, identity: demo::DemoIdentity) -> Result<(), String> {
    identity.validate()?;
    *state.0.lock().map_err(|_| "state unavailable")? = identity;
    Ok(())
}

fn main() {
    let sdk = tauri_plugin_busymate_ai::Builder::<tauri::Wry>::new()
        // The SDK reads this on every `state`/`mint`; only a salted hash reaches the chat.
        .account(|app| app.state::<DemoState>().current().account())
        // A fresh assertion per call, from YOUR authenticated backend.
        .mint(|app, request| {
            let identity = app.state::<DemoState>().current();
            demo::mint(identity, request)
        })
        // A fixed app-owned action set: `close` only when the setting says so.
        .on_action(|app, label, name, _params| {
            if name == "close" && app.state::<DemoState>().current().handle_close_action {
                let _ = app.busymate_ai().close_chat(Some(label));
                return true;
            }
            false
        })
        // The `close` fallback: dismiss the chat window.
        .on_close(|app, label| {
            let _ = app.busymate_ai().close_chat(Some(label));
        })
        .build();

    let builder = tauri::Builder::default()
        .manage(DemoState::default())
        .plugin(sdk)
        // Required for the microphone adapter: the webview's own media decision.
        .on_permission_request(tauri_plugin_busymate_ai::permission_handler)
        .invoke_handler(tauri::generate_handler![demo_apply_identity]);

    // macOS: WebKit's renderer termination is app-level in Tauri; let the SDK recover the chat.
    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(|webview| {
        tauri_plugin_busymate_ai::web_content_process_terminated(webview)
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running the Busymate SDK example");
}

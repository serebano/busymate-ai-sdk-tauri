//! Plugin commands. `open_chat`, `close_chat` and `account_changed` are for your
//! app's own local frontend (`busymate-ai:default`). `bridge` and `microphone`
//! are granted at runtime to the chat window's exact origins only, and refuse
//! any webview that is not a registered chat on an allowed origin.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Runtime, Webview};

use crate::bridge::{self, Outcome, MINT_CEILING_MS};
use crate::microphone::{self, Refusal};
use crate::{origin, os_permission, BusymateAiExt, ChatOptions, MintError, MintRequest};

#[tauri::command]
pub(crate) async fn open_chat<R: Runtime>(app: AppHandle<R>, options: ChatOptions) -> Result<String, String> {
    app.busymate_ai().open_chat(options).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn close_chat<R: Runtime>(app: AppHandle<R>, label: Option<String>) -> Result<(), String> {
    app.busymate_ai().close_chat(label.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn account_changed<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.busymate_ai().account_changed();
    Ok(())
}

fn document_origin<R: Runtime>(webview: &Webview<R>) -> Option<String> {
    webview.url().ok().and_then(|url| origin::origin_of_url(&url))
}

/// Identity wire v2: one envelope in, at most one reply envelope out.
#[tauri::command]
pub(crate) async fn bridge<R: Runtime>(webview: Webview<R>, envelope: String) -> Result<Option<String>, String> {
    let sdk = webview.busymate_ai().clone();
    let label = webview.label().to_string();
    let chat = sdk.chat(&label).ok_or("not a Busymate chat window")?;
    let core = chat.bridge.clone().ok_or("the identity bridge is not installed")?;
    let origin = document_origin(&webview);
    match core.handle(&envelope, origin.as_deref(), sdk.account()) {
        Outcome::Ignore => Ok(None),
        Outcome::Reply { op, reply } => {
            let denied = reply.contains(r#""e":"denied""#);
            if denied {
                sdk.notify(&label, "bridge-denied", origin.clone());
            } else {
                sdk.notify(&label, "bridge-op", Some(op.clone()));
            }
            if op == "hello" && !denied {
                chat.hello_seen.store(true, Ordering::SeqCst);
                if chat.pending_restored.swap(false, Ordering::SeqCst) {
                    let sdk = sdk.clone();
                    tauri::async_runtime::spawn(async move {
                        // After the hello reply has reached the page.
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        sdk.emit_event(&label, "restored");
                    });
                }
            }
            Ok(Some(reply))
        }
        Outcome::Mint { id, request } => {
            let Some(mint) = sdk.mint_fn() else {
                sdk.notify(&label, "mint", Some("not_signed_in".into()));
                return Ok(Some(bridge::error(&id, "not_signed_in")));
            };
            sdk.notify(&label, "mint", Some("requested".into()));
            let pending = mint(sdk.app_handle(), MintRequest::from_raw(request));
            // The ceiling only reclaims the callback; the chat owns every real budget.
            let (reply, outcome) = match tokio::time::timeout(Duration::from_millis(MINT_CEILING_MS), pending).await {
                Err(_) => (bridge::error(&id, "timeout"), "timeout"),
                Ok(Ok(Some(token))) => (bridge::ok(&id, json!({ "token": token.token, "nonce": token.nonce })), "token"),
                Ok(Ok(None)) | Ok(Err(MintError::NotSignedIn)) => (bridge::error(&id, "not_signed_in"), "not_signed_in"),
                Ok(Err(MintError::Failed(_))) => (bridge::error(&id, "mint_failed"), "mint_failed"),
            };
            sdk.notify(&label, "mint", Some(outcome.into()));
            Ok(Some(reply))
        }
        Outcome::Open { id, url, mode } => {
            let opened = sdk.open_external(&url, &mode);
            sdk.notify(&label, "external-open", Some(if opened { "opened" } else { "failed" }.into()));
            Ok(Some(bridge::ok(&id, json!({ "opened": opened }))))
        }
        Outcome::Action { id, name, params } => {
            let handled = sdk.handle_action(&label, &name, &params);
            sdk.notify(&label, "action", Some(format!("{name}:{}", if handled { "handled" } else { "unhandled" })));
            Ok(Some(bridge::ok(&id, json!({ "handled": handled }))))
        }
    }
}

/// Microphone adapter 1.0.0: asks the OS on the chat's user tap.
#[tauri::command]
pub(crate) async fn microphone<R: Runtime>(webview: Webview<R>, request: String) -> Result<Option<String>, String> {
    let sdk = webview.busymate_ai().clone();
    let label = webview.label().to_string();
    let chat = sdk.chat(&label).ok_or("not a Busymate chat window")?;
    if !chat.microphone {
        return Err("the microphone adapter is not installed".into());
    }
    let origin = document_origin(&webview);
    match microphone::parse_request(&request, origin.as_deref(), &chat.allowed) {
        Err(Refusal::Origin) => {
            sdk.notify(&label, "microphone-request", Some("refused-origin".into()));
            Err("origin not allowed".into())
        }
        Err(Refusal::Malformed) => Err("unsupported microphone request".into()),
        Ok(ask) => {
            let source = ask.source.as_deref().filter(|s| *s == "dictation" || *s == "voice").unwrap_or("other");
            sdk.notify(&label, "microphone-request", Some(source.to_string()));
            let granted = os_permission::request().await;
            sdk.notify(&label, "microphone-request", Some(if granted { "granted" } else { "denied" }.into()));
            Ok(Some(microphone::reply(&ask.id, granted)))
        }
    }
}

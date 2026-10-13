# Busymate AI SDK for Tauri — 1.0.1

Official independent Tauri v2 plugin for Busymate AI hosted chat:
`tauri-plugin-busymate-ai` (Rust crate) plus `tauri-plugin-busymate-ai-api`
(JavaScript bindings for your app's own frontend). Install once in your app;
hosted chat receives web updates independently.

The plugin opens the hosted chat in its own webview window, answers the
identity bridge (`busymate.bridge/2`, wire v2, immutable build 2.0.0) from your
app's **Rust** side, and adds the microphone adapter (wire 1, adapter 1.0.0)
that asks the OS only when the user taps the chat's microphone or voice control.

## Install

Requires Tauri **2.12** or later (the app-level `on_permission_request` hook)
and Rust 1.77.2+. Nothing is published to crates.io or npm; use the tagged
source or the release files.

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-busymate-ai = { git = "https://github.com/serebano/busymate-ai-sdk-tauri", tag = "1.0.1" }
```

Or download `tauri-plugin-busymate-ai-1.0.1.crate` from the
[1.0.1 release](https://github.com/serebano/busymate-ai-sdk-tauri/releases/tag/1.0.1),
extract it (`tar xzf …crate`) and depend on it with `path = "…"`.

JavaScript bindings (optional, for opening the chat from your frontend):

```sh
npm install ./tauri-plugin-busymate-ai-api-1.0.1.tgz @tauri-apps/api
```

## Integrate

```rust
use tauri_plugin_busymate_ai::{BusymateAiExt, ChatOptions, MintError, MintToken};

fn main() {
    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_busymate_ai::Builder::new()
                // Your signed-in account id, or None. Only SHA-256(appId:id) reaches the chat.
                .account(|_app| my_session::current_user_id())
                // A fresh single-use assertion from YOUR authenticated backend, every call.
                .mint(|_app, request| async move {
                    my_backend::mint_busymate(&request.nonce, request.aud.as_deref(),
                        &request.assistant, &request.origin)
                        .await
                        .map_err(|e| MintError::Failed(e.to_string()))
                })
                .on_close(|app, label| { let _ = app.busymate_ai().close_chat(Some(label)); })
                .build(),
        )
        // REQUIRED for the microphone: the webview's own media decision.
        .on_permission_request(tauri_plugin_busymate_ai::permission_handler);

    // macOS: let the SDK recover the chat after WebKit's content process dies.
    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(|webview| {
        tauri_plugin_busymate_ai::web_content_process_terminated(webview)
    });

    builder
        .setup(|app| {
            app.busymate_ai().open_chat(ChatOptions {
                url: "https://acme.busymate.ai/".into(), // your tenant's actual chat URL
                assistant: Some("acme".into()),
                origins: vec![],                        // extra exact https origins, if any
                ..Default::default()
            })?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
```

Add `"busymate-ai:default"` to your **local** window's capability if your
frontend calls `openChat` / `closeChat` / `accountChanged`. Do not add
`allow-bridge` or `allow-microphone` to any capability: the plugin grants those
two commands at runtime, only to the chat window and only for its exact https
origins. On macOS add `NSMicrophoneUsageDescription` to `src-tauri/Info.plist`
(see [microphone integration](docs/microphone.md)).

1. Read [identity and authentication](docs/identity.md).
2. Configure the [complete SDK reference](docs/configuration.md).
3. Follow [microphone and voice integration](docs/microphone.md), including the per-OS notes.
4. Validate on real Windows, macOS and Linux machines with the acceptance checklist; no OS prompt should occur merely on opening chat.

## Run the example app

`example-app/` is a real Tauri app that consumes this plugin and its JS bindings
from the repository. It has a settings view (chat URL, locale, assistant,
exact origins, guest vs identified with your mint endpoint, identity bridge
on/off, microphone on/off, close handling) and a callbacks log. See the
[example guide](example-app/README.md).

```sh
cd example-app
npm ci
npm run tauri dev          # or: npm run build && cargo run -p busymate-ai-tauri-example
```

The default guest chat is
`https://busymate.ai/support/busyproxy?channel=desktop_sdk&locale=en`.
`desktop_sdk` is the hosted product's channel for third-party desktop apps on
the official SDKs: the chat is the window's top-level page with the normal
chat layout (no Console title-bar inset), and `locale` applies from the first
frame. Do not use `embed` in a native window (it needs an allow-listed parent
frame and refuses to start top-level) or `desktop` (Busymate's own Console
app). Integration guide: [busymate.ai/docs/guides/tauri-sdk](https://busymate.ai/docs/guides/tauri-sdk).

## Versions and ownership

Platform distribution 1.0.1 is distinct from identity wire 2/build 2.0.0 and
microphone wire 1/adapter 1.0.0. The [release manifest](release-manifest.json)
pins the wire-relevant sources and the verbatim [v2 wire contract](CONTRACT-v2.md);
`python3 scripts/verify-release.py` checks them. This repository is canonical
for the Tauri SDK. Frozen web/shared contracts remain owned by the hosted
product; busymate.ai publishes the human integration guides.

`sdk-check` builds, lints and unit-tests the plugin and builds and tests the
example app on Ubuntu, macOS and Windows runners. It does not certify OS
permission dialogs, the hosted chat's live behavior inside a Tauri window, app
store or notarized distribution, or your backend integration. See
[changelog](CHANGELOG.md).

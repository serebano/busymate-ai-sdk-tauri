# Identity and authentication

The Tauri SDK implements the identity bridge `busymate.bridge/2` (wire v2,
immutable build 2.0.0) — the same five operations, envelope, errors and events
as the iOS, Android and JavaScript hosts. The byte-frozen
[v2 wire contract](../CONTRACT-v2.md) is the reference. There is no v1 bridge
in this SDK.

## How the chat reaches your app

The plugin injects, into the **main frame** of the chat window only, a
`window.BusymateBridge` object with the same shape the Android WebView and the
Electron preload expose: `postMessage(envelopeString)` plus
`addEventListener("message")`. Each envelope travels over Tauri IPC to the
plugin command `bridge`; the reply comes back as a `message` event. Events
(`resume`, `state`, `restored`) are dispatched as a `busymate-bridge`
CustomEvent on `window`. `hello` reports `platform: "tauri"` and
`transport: "tauri-ipc"`.

Who may ask is enforced twice:

1. **Tauri ACL.** When a chat opens, the plugin adds a runtime capability for
   that window only, with `remote.urls` set to its exact allowed origins
   (`https://<origin>/*`) and only `busymate-ai:allow-bridge` (and
   `allow-microphone` when enabled). Any other page, and every other command,
   is refused by Tauri before the plugin runs.
2. **The bridge.** The sender is the chat webview's top document origin
   (Tauri reports the top page, as React Native and Flutter do; the glue is not
   injected into sub-frames). Anything but exactly `https://<assistant>.busymate.ai`
   or an origin you listed is answered `denied`, and your mint is never called.

## What you write (Rust)

```rust
tauri_plugin_busymate_ai::Builder::new()
    .account(|app| app.state::<Session>().user_id())   // Option<String>; None = signed out
    .mint(|app, request| {
        let api = app.state::<Api>().inner().clone();
        async move { api.mint_busymate(request).await }   // Result<Option<MintToken>, MintError>
    })
    .on_action(|app, label, name, params| false)          // optional, a FIXED app-owned set
    .on_close(|app, label| { /* dismiss the chat */ })    // optional `close` fallback
    .build()
```

- `account` is read on every `state` and `mint`. Only
  SHA-256(`appId:accountId`) truncated to 128 bits (hex) reaches the chat; the
  raw id never leaves the app. `appId` is your Tauri `identifier` unless you set
  `Builder::app_info`.
- `mint` receives `MintRequest { nonce, aud, reason, assistant, origin, raw }`.
  `assistant` and `origin` are set by the plugin, never by the page, so your
  backend may bind the token to them. Return `Ok(Some(MintToken { token, nonce }))`
  with the requested nonce, `Ok(None)` or `Err(MintError::NotSignedIn)` when
  nobody is signed in, `Err(MintError::Failed(_))` otherwise. A 15-second
  ceiling answers `timeout`; the chat owns every real budget.
- Your authenticated backend mints a fresh single-use launch assertion for each
  call. Never cache or reuse a token/nonce pair. Never put a signing key,
  Busymate admin key or service-role credential in the app. Use your existing
  authenticated HTTPS API; do not invent a public Busymate mint endpoint (see
  the official [identity guide](https://busymate.ai/docs/guides/mobile-in-app-support)).
- After sign-in, sign-out or an account switch call
  `app.busymate_ai().account_changed()` (or `accountChanged()` from JS). The
  bridge emits `state`; hosted chat owns refresh and clearing.
- `open` (https only) goes to the system browser for both `external` and `auth`
  modes; replace it with `Builder::open_url` if you run your own auth flow.
  There is no in-app auth session, so `hello` does not announce `ev:auth`.
- `action`: return true only for actions you handle. `close` falls back to
  `on_close`; without it `close` is answered `handled: false`. Never expose
  arbitrary native execution.

A working guest chat is always the floor: identity failures keep the chat
anonymous (`not_signed_in`, `mint_failed`, `timeout`, `unsupported`). Never log
tokens, nonces, raw account ids or audio; the plugin's `ChatEvent`s never carry
them.

## Lifecycle events

| Event | Tauri trigger |
|---|---|
| `resume` | The chat window gains focus (after its first `hello`). |
| `state` | You call `account_changed()`. |
| `restored` | The chat reloaded after its web content process died: WebView2 `ProcessFailed` (render process exited/unresponsive) on Windows, WebKitGTK `web-process-terminated` on Linux, and on macOS your `on_web_content_process_terminate` hook forwarding to `web_content_process_terminated`. Emitted after the next `hello`. |

Events are delivered only after `hello` and only while the chat document is on
an allowed origin.

## Navigation

The chat window allows navigation only to its exact allowed https origins (and
`about:`). Pop-ups (`window.open`, `target=_blank`) never become in-app windows:
https URLs open in the system browser. Keep the chat URL on the tenant's actual
chat origin; a marketing page is not a chat URL.

## Known interaction with the hosted product

busymate.ai treats a page that has `window.__TAURI_INTERNALS__` as Busymate's
own macOS Console shell (desktop chrome and its `desktop_*` shell commands).
Tauri always installs that object, and in a tenant app those commands are
refused by the ACL, so the hosted page degrades to its web behavior. Until the
hosted product distinguishes its own shell (it also stamps a
`BusymateAIConsole/` user agent), verify link opening and window chrome in your
app; this is tracked as a hosted-product follow-up.

# Tauri SDK example app

A real Tauri v2 app that consumes the **local** `tauri-plugin-busymate-ai`
crate (`../plugin`) and its JS bindings (`../plugin/guest-js`). There are no
copied or mock SDK implementations. It defaults to identity bridge v2 build
2.0.0 and microphone adapter 1.0.0.

## Build and run

Prerequisites: Rust stable, Node 20+, and the
[Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS
(WebView2 on Windows; `libwebkit2gtk-4.1-dev` and friends on Linux).

```sh
cd example-app
npm ci
npm run tauri dev
```

Without the Tauri CLI: `npm run build && cargo run -p busymate-ai-tauri-example`
from the repository root. `cargo test -p busymate-ai-tauri-example` runs the
settings/backend-mapping tests. CI (`sdk-check`) builds the app with
`cargo build` on Ubuntu, macOS and Windows; it does not launch it.

## Settings

The main window is the settings view; **Apply and open chat** validates the
settings, closes any open chat and opens a new chat window through the SDK.

| Control | What it configures |
|---|---|
| Chat HTTPS URL | `ChatOptions.url` (default `https://busymate.ai/support/busyproxy?channel=desktop_sdk&locale=en`). |
| Locale | Sets the URL's `locale` parameter. |
| Assistant slug | `ChatOptions.assistant` (default `busyproxy`). |
| Exact HTTPS origins | `ChatOptions.origins` (default `https://busymate.ai`, the hosted `/support/<slug>` origin). Must include the chat URL's origin; no wildcards or paths. |
| Install the identity bridge | `ChatOptions.identity`. |
| Guest / Identified | The example's `account` callback returns nothing (guest) or the account ID. An entered ID is **not** a login. |
| Authenticated mint endpoint, backend session token | The example's `mint` callback POSTs `{nonce, aud, reason, assistant, origin}` to YOUR https endpoint, with the optional bearer token (memory only). 401/403 = signed out; redirects are refused; the reply must be `{"token":"…","nonce":"<same nonce>"}`. |
| Handle `close` through onAction | The example's `on_action` handles `close` only when checked; otherwise the `on_close` fallback closes the window. |
| Enable the tap-triggered OS permission adapter | `ChatOptions.microphone`. |
| accountChanged() | Applies the identity fields and calls the SDK's `accountChanged()`. |
| Reset | Guest defaults. Does not reset an OS microphone permission. |

The **Callbacks** list shows the SDK's `ChatEvent`s (opened, page-load,
bridge ops, mint outcome, actions, microphone requests and media decisions,
navigation refusals, renderer recovery). It never shows tokens, nonces, account
IDs, envelopes or audio, holds at most 100 entries and is not persisted.

## Live guest flow

The default is the real hosted guest chat for the `busyproxy` assistant — not
an authenticated example workspace or a stub. Its features belong to that
tenant. Guest mode makes no mint request. Identified mode needs your real
assistant/chat URL and a real authenticated tenant backend; this example
contains no backend, signing key or fabricated identity.

On macOS, `src-tauri/Info.plist` carries `NSMicrophoneUsageDescription`.
Microphone dialogs, renderer crashes and the hosted chat's behavior inside this
window have not been exercised on physical machines for 1.0.0; use the
[acceptance checklist](../docs/microphone.md#acceptance-checklist-for-your-app-release).

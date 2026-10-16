# Changelog

## 1.0.2

Release packaging: the microphone adapter source (`microphone.rs` sha256 `2f2156f4c59015490fbad03930d078574ceb24f2fad5e82b7553bde9fcc7f458`; adapter 1.0.0, wire 1) is attached to every GitHub release and listed in `SHA256SUMS`; docs link the release ([Download the adapter](docs/microphone.md#download-the-adapter)). `release-manifest.json` lists `adapterAssets` and `scripts/verify-release.py` checks them. No adapter or bridge code change (refs serebano/busymate-ai-src#2156).

## 1.0.1

Default guest chat now uses `channel=desktop_sdk`
(`https://busymate.ai/support/busyproxy?channel=desktop_sdk&locale=en`), the
hosted product's channel for third-party desktop SDK apps: top-level window,
normal chat layout with no Console title-bar inset, locale applied on the
first frame. `channel=embed` (1.0.0) needs an allow-listed parent frame and
fails as a top-level native window. busymate.ai no longer treats a Tauri
window as its own Console app (serebano/busymate-ai-src#1849). Docs link the
busymate.ai Tauri guide. Identity wire 2/build 2.0.0 and microphone adapter
1.0.0 are unchanged; `lib.rs` and the JS bindings change, so this is a new
version.

## 1.0.0

First independent Tauri v2 distribution: `tauri-plugin-busymate-ai` (Rust) and
`tauri-plugin-busymate-ai-api` (JavaScript bindings). Hosted chat in its own
webview window; identity bridge `busymate.bridge/2` (wire v2, build 2.0.0)
answered from the app's Rust side; microphone adapter (wire 1, adapter 1.0.0)
with tap-only OS permission on macOS (AVFoundation), Windows (privacy consent
switches) and Linux; WebView2/WKWebView/WebKitGTK media decisions through
Tauri's `on_permission_request`; runtime IPC capability scoped to the chat
window and its exact origins; navigation allowlist; renderer recovery with
`restored`. Requires Tauri 2.12+. Default guest chat uses `channel=embed`.
Hosted chat receives updates separately; your app release is needed to install
or upgrade native SDK code.

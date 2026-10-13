# Changelog

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

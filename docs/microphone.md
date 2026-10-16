# Native microphone integration for Tauri — adapter 1.0.0

The user taps the chat's microphone or voice control; the chat sends a request
to the app; the app asks the operating system; the chat starts capture only
after permission is granted. No separate native permission button is needed,
and nothing is asked when the chat merely opens.

## Enable it

1. `ChatOptions.microphone = true` (the default). The plugin injects
   `window.BusymateMicrophone` into the chat's main frame and grants the
   `microphone` command to the chat's exact origins.
2. Register the media decision on your app builder — **required**:

   ```rust
   tauri::Builder::default()
       .on_permission_request(tauri_plugin_busymate_ai::permission_handler)
   ```

   Without it, wry's macOS default grants every capture request in every
   webview. With it, a chat window gets the microphone only when the adapter is
   enabled, its document is on an allowed origin and the OS permission is
   currently granted; camera and screen capture are denied.
3. Per-OS setup below.

This is native app code: integrating it requires **your app's own release**.

## Wire (identical to iOS and Android)

Request (chat → app), a JSON string, at most 4096 bytes:

```json
{"type":"busymate.microphone.v1.request","id":"mic-unique-id","source":"dictation"}
```

`source` is `dictation` or `voice`. Reply (app → chat) as a `message` event on
`window.BusymateMicrophone`, the same id and a boolean:

```json
{"id":"mic-unique-id","granted":true}
```

A request from a non-allowed origin is refused by Tauri's ACL or the plugin;
the injected glue then answers `granted:false` for that id, as the iOS
adapter's rejected reply does, so the chat shows its blocked notice instead of
waiting.

## Per operating system

| | OS permission (asked on tap) | Webview media decision | What you add |
|---|---|---|---|
| **macOS** (WKWebView) | AVFoundation `AVCaptureDevice` audio authorization. Not determined → the system prompt; authorized → immediate; denied/restricted → `granted:false` without a prompt. | WKUIDelegate `requestMediaCapturePermission`, through Tauri's `on_permission_request`. | `NSMicrophoneUsageDescription` in `src-tauri/Info.plist` (Tauri merges it into the bundle). Without it the adapter refuses rather than ask, because macOS terminates an app that requests access without the key. Hardened-runtime/notarized builds also need the `com.apple.security.device.audio-input` entitlement; sandboxed (App Store) builds need it too. |
| **Windows** (WebView2) | Desktop apps have no per-app runtime prompt API. The adapter reads Settings › Privacy & security › Microphone: device access, "Microphone access" and "Let desktop apps access your microphone" (`CapabilityAccessManager\ConsentStore\microphone`); any of them set to Deny → `granted:false`. Windows may show its own first-use notice when capture starts. | WebView2 `PermissionRequested` (microphone), through `on_permission_request`; the plugin's answer suppresses WebView2's own prompt. | Nothing. |
| **Linux** (WebKitGTK) | No OS-level microphone permission for unsandboxed apps: the answer is granted. Flatpak/Snap portals are not integrated in 1.0.0. | WebKitGTK `permission-request` (user media, audio only), through `on_permission_request`. Tauri's Linux default for anything unanswered is deny. | A working PulseAudio/PipeWire input. |

Every activation rechecks the OS state, so a permission revoked in system
settings is seen at the next tap. The adapter does not open system settings.

## Retry, revoke, cancel

The chat owns the user flow: a refusal, invalid reply or 60-second timeout shows
its blocked notice; **Try again** asks the app again; concurrent asks share one
pending request. After a macOS denial the OS does not prompt again: the user
grants access in System Settings › Privacy & Security › Microphone and taps
**Try again**. Cancelling dictation or leaving voice mode while permission is
pending means a late grant cannot start capture. Cancelling does not dismiss an
OS prompt or undo an OS grant.

## Download the adapter

`microphone.rs` (microphone adapter 1.0.0, wire 1) ships in this SDK and is attached to every GitHub release, with `SHA256SUMS`:

- Latest release: https://github.com/serebano/busymate-ai-sdk-tauri/releases/latest/download/microphone.rs
- Pinned, SDK 1.0.2: https://github.com/serebano/busymate-ai-sdk-tauri/releases/download/1.0.2/microphone.rs
- Checksums: https://github.com/serebano/busymate-ai-sdk-tauri/releases/download/1.0.2/SHA256SUMS (`shasum -a 256 -c SHA256SUMS --ignore-missing`)

The SDK installs the adapter itself; the standalone file is for audit and vendoring.

This repository and its releases are the only source of the adapter. busymate.ai no longer publishes adapter copies; its legacy `https://busymate.ai/sdk/microphone/1.0.0/` files stay frozen for existing integrations.

## Troubleshooting

| Symptom | Check |
|---|---|
| No prompt on first tap (macOS) | Permission may already be decided (`tccutil reset Microphone <bundle id>` resets it). Confirm `NSMicrophoneUsageDescription`, `microphone: true`, and that the chat URL's origin is in the allowed set. |
| OS allowed, chat still cannot record | Confirm `.on_permission_request(tauri_plugin_busymate_ai::permission_handler)` is registered and not shadowed by a webview-level handler; check the `media-decision` chat event. |
| Windows always refuses | Settings › Privacy & security › Microphone: enable microphone access and "Let desktop apps access your microphone". |
| Request times out | Check the `microphone-request` chat events and that the window was opened by the plugin (the glue exists only there). Never log audio or identity tokens. |
| Camera request denied | Expected: microphone only. |

## Acceptance checklist for your app release

Build your app and verify on real machines for each OS you ship:

1. Fresh permission state: open chat — no prompt. Tap **microphone**, see the
   OS prompt where the OS has one (macOS), allow, dictate. Repeat for **voice mode**.
2. Deny (macOS) or switch off desktop-app microphone access (Windows): the blocked
   notice appears, no capture, no prompt loop. Re-enable in system settings and
   use **Try again**.
3. Already granted: the next activation starts without a prompt. Revoke in
   system settings; the next activation is refused.
4. While permission is pending, cancel dictation or leave voice mode; accepting
   afterwards does not start capture.
5. Close and reopen the chat window and repeat; navigate the chat window to a
   non-allowed origin is refused.
6. Camera requests and other origins are refused.

`sdk-check` compiles the adapter on all three OSes and unit-tests the request
parsing and the injected glue. It cannot show OS dialogs: none of the
permission flows above have been exercised on a physical machine for 1.0.0.
Record those results with your own release.

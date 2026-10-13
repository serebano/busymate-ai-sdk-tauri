# Configuration reference

## Plugin builder (Rust)

| Setting | Behavior |
|---|---|
| `account(f)` | `Fn(&AppHandle) -> Option<String>`: the current account id, or `None` (signed out). Read on every `state`/`mint`; only its salted SHA-256 crosses. |
| `mint(f)` | `Fn(&AppHandle, MintRequest) -> impl Future<Output = Result<Option<MintToken>, MintError>>`: a fresh token/nonce from your authenticated backend, every call; never a cached assertion. Without it, `mint` is answered `not_signed_in`. |
| `on_action(f)` | Optional fixed app-action handler `(app, label, name, params) -> bool`; return true only when handled. Names used by the chat: `close`, `ready` (informational). |
| `on_close(f)` | Optional host dismissal `(app, label)`; the fallback for an unhandled `close`. |
| `open_url(f)` | Optional `(app, url, mode) -> bool` for https opens (`external`/`auth`). Default: the system browser. |
| `on_event(f)` | Optional observer of `ChatEvent`s in Rust. |
| `app_info(AppInfo)` | Optional `hello.app` override; `id` salts the account key. Default: Tauri `identifier` and `version`. |

App-level hooks you register on `tauri::Builder` (Tauri exposes them only there):

| Hook | Why |
|---|---|
| `.on_permission_request(tauri_plugin_busymate_ai::permission_handler)` | **Required for the microphone.** The webview's own media decision. Chat windows: microphone only, allowed origin only, OS permission granted; camera and screen capture denied. Other webviews and other kinds keep the platform default. If your app already has a handler, call `permission_handler` from it first and use its answer unless it is `PermissionResponse::Default`. |
| `.on_web_content_process_terminate(...)` (macOS) | Forward to `tauri_plugin_busymate_ai::web_content_process_terminated(webview)` so the chat reloads and receives `restored`. Windows and Linux are handled by the plugin. |

## `ChatOptions` (Rust `open_chat`, JS `openChat`)

| Field | Default | Behavior |
|---|---|---|
| `url` | `https://busymate.ai/support/busyproxy?channel=embed&locale=en` | The tenant's actual https chat URL, supported query parameters preserved (`channel`, `locale`, `theme`…). Credentials are refused. Use `channel=embed`: `desktop` is reserved for Busymate's own Console app. |
| `assistant` | none | Assistant slug; allows exactly `https://<assistant>.busymate.ai`. Required when `identity` is true. |
| `origins` | `[]` | Extra exact https origins (`https://host[:port]`): your mapped support host, or `https://busymate.ai` for the hosted `/support/<slug>` URL. No wildcards, paths, queries, fragments or credentials — an invalid entry is an error. The chat URL's own origin must be in the allowed set. |
| `identity` | `true` | Install `window.BusymateBridge` and grant `allow-bridge` to the chat's origins. |
| `microphone` | `true` | Install `window.BusymateMicrophone` and grant `allow-microphone`. When false the chat keeps its browser fallback and the permission handler denies the microphone. |
| `title`, `width`, `height` | `Support`, 420, 680 | The chat window. |

The allowed set (assistant origin + `origins`) is used for the identity bridge,
the microphone adapter, navigation and the runtime IPC capability. `open_chat`
returns the window label (`busymate-chat-<n>`); `close_chat(Some(label))` closes
one, `close_chat(None)` all. `account_changed()` emits `state` to every chat.

## Permissions and capabilities

| Identifier | Commands | Who gets it |
|---|---|---|
| `busymate-ai:default` | `open_chat`, `close_chat`, `account_changed` | Your local window, in your capability file. |
| `busymate-ai:allow-bridge` | `bridge` | Granted by the plugin at runtime to the chat window, `remote.urls` = its exact origins, `local: false`. |
| `busymate-ai:allow-microphone` | `microphone` | Same, only when `microphone` is true. |

Never put `allow-bridge` or `allow-microphone` in a static capability. The
commands also refuse any webview that is not a registered chat on an allowed
origin. Runtime capabilities cannot be removed in Tauri 2; each chat gets a new
label and its own capability, so an old capability only ever names a closed
window. Tauri 2.12 `dynamic-acl` (a default feature) is required.

## JavaScript bindings

`openChat(options)`, `closeChat(label?)`, `accountChanged()`,
`onChatEvent(handler)` (event `busymate-ai://chat`), `withLocale(url, locale)`,
plus `DEFAULT_CHAT_URL`, `SDK_VERSION`, `IDENTITY_BUILD`, `MICROPHONE_ADAPTER`.
Events carry `{label, kind, detail}` and never tokens, nonces, account ids,
envelopes or audio. They are not emitted to chat windows.

## What is not configurable

Hosted chat appearance, theme, locale, voice availability and tenant behavior
are configured through the hosted chat URL and Busymate Console, not by native
flags. There is no SDK setting for automatic OS prompts on page load, camera
access, opening system settings, timeout overrides, audio routing, provider
credentials or token caching. Window chrome, layout and audio routing are your
host's.

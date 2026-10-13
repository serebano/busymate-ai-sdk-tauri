# tauri-plugin-busymate-ai-api 1.0.1

JavaScript bindings for [`tauri-plugin-busymate-ai`](https://github.com/serebano/busymate-ai-sdk-tauri),
for your app's **own local frontend**. Install the release tarball (no npm registry
package is published):

```sh
npm install ./tauri-plugin-busymate-ai-api-1.0.1.tgz @tauri-apps/api
```

```js
import { openChat, closeChat, accountChanged, onChatEvent, DEFAULT_CHAT_URL } from 'tauri-plugin-busymate-ai-api'

await onChatEvent((event) => console.log(event.kind, event.detail))
const label = await openChat({ url: DEFAULT_CHAT_URL, assistant: 'busyproxy', origins: ['https://busymate.ai'] })
```

Grant `busymate-ai:default` to your local window's capability. The identity
`account`/`mint` callbacks are Rust-only, on the plugin builder. See the
repository README and `docs/`.

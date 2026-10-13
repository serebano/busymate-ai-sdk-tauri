// Settings view of the Busymate AI SDK example. Everything here goes through
// the REAL SDK: the JS bindings (tauri-plugin-busymate-ai-api) open/close the
// chat and forward accountChanged(); identity callbacks run in this app's Rust
// side (src-tauri/src/main.rs), which is where a real app's mint belongs.
import { invoke } from '@tauri-apps/api/core'
import {
  accountChanged,
  closeChat,
  DEFAULT_CHAT_URL,
  IDENTITY_BUILD,
  MICROPHONE_ADAPTER,
  onChatEvent,
  openChat,
  SDK_VERSION,
  withLocale,
} from 'tauri-plugin-busymate-ai-api'

const DEFAULTS = Object.freeze({
  url: DEFAULT_CHAT_URL,
  locale: 'en',
  assistant: 'busyproxy',
  origins: 'https://busymate.ai',
  identity: true,
  mode: 'guest',
  accountId: '',
  mintUrl: '',
  sessionToken: '',
  handleCloseAction: false,
  microphone: true,
})

const form = document.getElementById('settings')
const errorBox = document.getElementById('error')
const log = document.getElementById('log')
const MAX_LOG = 100

document.getElementById('versions').textContent =
  `SDK ${SDK_VERSION} · identity wire v2 build ${IDENTITY_BUILD} · microphone adapter ${MICROPHONE_ADAPTER}`

function addLog(message) {
  const item = document.createElement('li')
  item.textContent = `${new Date().toLocaleTimeString()} · ${message}`
  log.prepend(item)
  while (log.children.length > MAX_LOG) log.lastElementChild.remove()
}

function fill(values) {
  for (const [name, value] of Object.entries(values)) {
    const field = form.elements[name]
    if (!field) continue
    if (field instanceof RadioNodeList) {
      for (const radio of field) radio.checked = radio.value === value
    } else if (field.type === 'checkbox') {
      field.checked = Boolean(value)
    } else {
      field.value = value
    }
  }
}

function read() {
  const data = new FormData(form)
  const text = (name) => String(data.get(name) ?? '').trim()
  return {
    url: text('url'),
    locale: text('locale'),
    assistant: text('assistant'),
    origins: text('origins').split(/[\n,]/).map((value) => value.trim()).filter(Boolean),
    identity: form.elements.identity.checked,
    signedIn: text('mode') === 'identified',
    accountId: text('accountId'),
    mintUrl: text('mintUrl'),
    sessionToken: String(data.get('sessionToken') ?? ''),
    handleCloseAction: form.elements.handleCloseAction.checked,
    microphone: form.elements.microphone.checked,
  }
}

function demoIdentity(settings) {
  return {
    signedIn: settings.signedIn,
    accountId: settings.accountId,
    mintUrl: settings.mintUrl,
    sessionToken: settings.sessionToken,
    handleCloseAction: settings.handleCloseAction,
  }
}

async function apply() {
  errorBox.textContent = ''
  const settings = read()
  try {
    const url = withLocale(settings.url, settings.locale)
    // Rust validates the identity settings (no login, no fabricated identity).
    await invoke('demo_apply_identity', { identity: demoIdentity(settings) })
    await closeChat()
    const label = await openChat({
      url,
      assistant: settings.assistant || null,
      origins: settings.origins,
      identity: settings.identity,
      microphone: settings.microphone,
      title: 'Support chat',
    })
    addLog(`Opened ${label}: identity ${settings.identity ? 'v2 installed' : 'off'}, ` +
      `${settings.signedIn ? 'identified mode' : 'guest'}, microphone ${settings.microphone ? 'enabled' : 'disabled'}`)
  } catch (error) {
    errorBox.textContent = String(error)
    addLog('Configuration rejected')
  }
}

form.addEventListener('submit', (event) => {
  event.preventDefault()
  void apply()
})

document.getElementById('close').addEventListener('click', async () => {
  await closeChat()
  addLog('Chat closed by the host')
})

document.getElementById('account').addEventListener('click', async () => {
  errorBox.textContent = ''
  try {
    await invoke('demo_apply_identity', { identity: demoIdentity(read()) })
    await accountChanged()
    addLog('accountChanged() forwarded; no identity fabricated')
  } catch (error) {
    errorBox.textContent = String(error)
  }
})

document.getElementById('reset').addEventListener('click', async () => {
  fill(DEFAULTS)
  await invoke('demo_apply_identity', { identity: demoIdentity(read()) }).catch(() => {})
  addLog('Settings reset to guest defaults (OS microphone permission is not reset)')
})

document.getElementById('clear').addEventListener('click', () => { log.textContent = '' })

void onChatEvent((event) => {
  addLog(`${event.label} · ${event.kind}${event.detail ? ` · ${event.detail}` : ''}`)
})

fill(DEFAULTS)

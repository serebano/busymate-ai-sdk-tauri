// Busymate AI SDK for Tauri — JavaScript bindings for YOUR app's local frontend.
// The hosted chat window never loads this file: it talks to the plugin through
// the glue the plugin injects (identity wire v2, microphone adapter 1.0.0).
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/** Default guest chat. `channel=desktop_sdk` is the hosted product's channel for third-party desktop SDK apps. */
export const DEFAULT_CHAT_URL = 'https://busymate.ai/support/busyproxy?channel=desktop_sdk&locale=en'
/** The event the plugin emits to your frontend. */
export const CHAT_EVENT = 'busymate-ai://chat'
export const SDK_VERSION = '1.0.2'
export const IDENTITY_BUILD = '2.0.0'
export const MICROPHONE_ADAPTER = '1.0.0'

/**
 * Open the hosted chat in its own window. Resolves to the window label.
 * Identity callbacks (`account`, `mint`) are configured in Rust on the plugin builder.
 * @param {import('./index').ChatOptions} options
 * @returns {Promise<string>}
 */
export function openChat(options) {
  return invoke('plugin:busymate-ai|open_chat', { options })
}

/** Close one chat window, or every chat window when `label` is omitted. */
export function closeChat(label) {
  return invoke('plugin:busymate-ai|close_chat', { label: label ?? null })
}

/** Tell open chats the signed-in account changed (sign-in, sign-out, switch). */
export function accountChanged() {
  return invoke('plugin:busymate-ai|account_changed')
}

/**
 * Observe host-side chat events (no tokens, nonces, account ids or audio).
 * @param {(event: import('./index').ChatEvent) => void} handler
 * @returns {Promise<() => void>} unlisten
 */
export function onChatEvent(handler) {
  return listen(CHAT_EVENT, (event) => handler(event.payload))
}

/**
 * The chat URL with its `locale` query parameter set, other parameters preserved.
 * @param {string} url
 * @param {string} locale
 */
export function withLocale(url, locale) {
  const parsed = new URL(url)
  if (locale) parsed.searchParams.set('locale', locale)
  else parsed.searchParams.delete('locale')
  return parsed.toString()
}

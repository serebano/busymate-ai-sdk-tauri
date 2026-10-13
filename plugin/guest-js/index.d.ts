export declare const DEFAULT_CHAT_URL: string
export declare const CHAT_EVENT: 'busymate-ai://chat'
export declare const SDK_VERSION: '1.0.1'
export declare const IDENTITY_BUILD: '2.0.0'
export declare const MICROPHONE_ADAPTER: '1.0.0'

export interface ChatOptions {
  /** The tenant's https chat URL, query parameters included. */
  url?: string
  /** Assistant slug: allows exactly https://<assistant>.busymate.ai. Required for identity. */
  assistant?: string | null
  /** Extra EXACT https origins (no paths, wildcards or credentials). */
  origins?: string[]
  /** Install the identity bridge (default true). */
  identity?: boolean
  /** Install the microphone adapter (default true). */
  microphone?: boolean
  title?: string
  width?: number
  height?: number
}

export interface ChatEvent {
  label: string
  kind:
    | 'opened' | 'closed' | 'page-load' | 'navigation-refused' | 'external-open'
    | 'bridge-op' | 'bridge-denied' | 'bridge-event' | 'mint' | 'action' | 'close'
    | 'microphone-request' | 'media-decision' | 'renderer-restored'
  detail: string | null
}

export declare function openChat(options: ChatOptions): Promise<string>
export declare function closeChat(label?: string): Promise<void>
export declare function accountChanged(): Promise<void>
export declare function onChatEvent(handler: (event: ChatEvent) => void): Promise<() => void>
export declare function withLocale(url: string, locale: string): string

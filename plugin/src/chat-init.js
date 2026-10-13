// Busymate AI SDK for Tauri — chat document glue (identity wire v2 build 2.0.0,
// microphone wire 1 / adapter 1.0.0). Injected by tauri-plugin-busymate-ai into
// the MAIN FRAME of the chat window only, before the chat's own scripts run.
//
// It exposes the same object shape the Android WebView and the Electron preload
// inject — `postMessage(string)` plus `addEventListener("message")` — so the
// chat served from busymate.ai speaks to every host the same way:
//   window.BusymateBridge      -> plugin command `bridge`     (identity v2)
//   window.BusymateMicrophone  -> plugin command `microphone` (adapter 1.0.0)
// Replies arrive as `message` events on the object. Bridge EVENTS (resume,
// state, restored) are dispatched by the plugin as a `busymate-bridge`
// CustomEvent on window. Nothing here decides policy: the Rust side checks the
// sender's origin again, and Tauri's ACL lets only the exact allowed https
// origins reach these two commands at all.
;(function (config) {
  'use strict'
  var internals = window.__TAURI_INTERNALS__
  if (!internals || typeof internals.invoke !== 'function') return
  if (config.origins.indexOf(window.location.origin) < 0) return
  var invoke = internals.invoke

  function port(command, argument, onFailure) {
    var listeners = []
    function deliver(data) {
      listeners.slice().forEach(function (listener) {
        try { listener({ data: data }) } catch (_) { /* a page listener's fault is its own */ }
      })
    }
    return Object.freeze({
      postMessage: function (value) {
        var text = String(value)
        var args = {}
        args[argument] = text
        Promise.resolve()
          .then(function () { return invoke('plugin:busymate-ai|' + command, args) })
          .then(function (reply) {
            if (typeof reply === 'string') deliver(reply)
          }, function () {
            var fallback = onFailure ? onFailure(text) : null
            if (fallback) deliver(fallback)
          })
      },
      addEventListener: function (type, listener) {
        if (type === 'message' && typeof listener === 'function' && listeners.indexOf(listener) < 0) listeners.push(listener)
      },
      removeEventListener: function (type, listener) {
        var index = listeners.indexOf(listener)
        if (index >= 0) listeners.splice(index, 1)
      }
    })
  }

  function define(name, value) {
    if (Object.prototype.hasOwnProperty.call(window, name)) return
    Object.defineProperty(window, name, { value: value, configurable: false, enumerable: false, writable: false })
  }

  if (config.identity) define('BusymateBridge', port('bridge', 'envelope'))
  if (config.microphone) {
    // A refused request (wrong origin, host error) is a correlated `granted:false`,
    // like the iOS adapter's rejected reply: the chat shows its blocked notice.
    define('BusymateMicrophone', port('microphone', 'request', function (text) {
      try {
        var request = JSON.parse(text)
        if (request && typeof request.id === 'string') return JSON.stringify({ id: request.id, granted: false })
      } catch (_) { /* not a request */ }
      return null
    }))
  }
})(__BUSYMATE_CONFIG__)

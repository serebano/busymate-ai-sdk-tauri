// Runs the REAL injected glue (plugin/src/chat-init.js) in a VM with a fake
// Tauri IPC, and drives it the way the hosted chat does (bridgeV2.ts and
// nativeMicrophone.ts in the hosted product).
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import vm from 'node:vm'

const source = readFileSync(new URL('../src/chat-init.js', import.meta.url), 'utf8')

function run({ origin = 'https://busymate.ai', config, invoke, internals = true } = {}) {
  const calls = []
  const window = {
    location: { origin },
    __TAURI_INTERNALS__: internals ? { invoke: (cmd, args) => { calls.push({ cmd, args }); return invoke(cmd, args) } } : undefined,
  }
  const cfg = config ?? { origins: ['https://busymate.ai', 'https://busyproxy.busymate.ai'], identity: true, microphone: true }
  vm.runInNewContext(source.replace('__BUSYMATE_CONFIG__', JSON.stringify(cfg)), { window, JSON, Promise, Object, String })
  return { window, calls }
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

test('exposes the Android/Electron object shape on allowed origins only', () => {
  const { window } = run({ invoke: async () => null })
  assert.equal(typeof window.BusymateBridge.postMessage, 'function')
  assert.equal(typeof window.BusymateBridge.addEventListener, 'function')
  assert.equal(typeof window.BusymateMicrophone.postMessage, 'function')
  assert.ok(Object.isFrozen(window.BusymateBridge))
  const foreign = run({ origin: 'https://evil.example', invoke: async () => null }).window
  assert.equal(foreign.BusymateBridge, undefined)
  assert.equal(foreign.BusymateMicrophone, undefined)
  assert.equal(run({ internals: false }).window.BusymateBridge, undefined)
})

test('installs only what is enabled', () => {
  const { window } = run({ config: { origins: ['https://busymate.ai'], identity: false, microphone: true }, invoke: async () => null })
  assert.equal(window.BusymateBridge, undefined)
  assert.equal(typeof window.BusymateMicrophone.postMessage, 'function')
  const quiet = run({ config: { origins: ['https://busymate.ai'], identity: true, microphone: false }, invoke: async () => null }).window
  assert.equal(quiet.BusymateMicrophone, undefined)
})

test('bridge: one envelope string in, the reply as a message event', async () => {
  const reply = '{"bm":2,"id":"b1","ok":true,"r":{"bridge":"busymate"}}'
  const { window, calls } = run({ invoke: async () => reply })
  const seen = []
  window.BusymateBridge.addEventListener('message', (event) => seen.push(event.data))
  window.BusymateBridge.postMessage(JSON.stringify({ bm: 2, id: 'b1', op: 'hello', p: {} }))
  await tick(); await tick()
  assert.deepEqual(JSON.parse(JSON.stringify(calls)), [{ cmd: 'plugin:busymate-ai|bridge', args: { envelope: '{"bm":2,"id":"b1","op":"hello","p":{}}' } }])
  assert.deepEqual(seen, [reply])
})

test('bridge: an ignored envelope or a refused IPC produces no reply', async () => {
  let answer = async () => null
  const { window } = run({ invoke: () => answer() })
  const seen = []
  window.BusymateBridge.addEventListener('message', (event) => seen.push(event.data))
  window.BusymateBridge.postMessage('garbage')
  answer = async () => { throw new Error('ACL') }
  window.BusymateBridge.postMessage('{"bm":2,"id":"x","op":"state"}')
  await tick(); await tick()
  assert.deepEqual(seen, [])
})

test('microphone: correlated reply, and a refusal becomes granted:false', async () => {
  let answer = async () => '{"granted":true,"id":"mic-1"}'
  const { window, calls } = run({ invoke: () => answer() })
  const seen = []
  const listener = (event) => seen.push(JSON.parse(event.data))
  window.BusymateMicrophone.addEventListener('message', listener)
  window.BusymateMicrophone.postMessage(JSON.stringify({ type: 'busymate.microphone.v1.request', id: 'mic-1', source: 'voice' }))
  await tick(); await tick()
  assert.equal(calls[0].cmd, 'plugin:busymate-ai|microphone')
  assert.deepEqual(seen, [{ granted: true, id: 'mic-1' }])
  answer = async () => { throw new Error('origin not allowed') }
  window.BusymateMicrophone.postMessage(JSON.stringify({ type: 'busymate.microphone.v1.request', id: 'mic-2', source: 'dictation' }))
  await tick(); await tick()
  assert.deepEqual(seen[1], { id: 'mic-2', granted: false })
  window.BusymateMicrophone.removeEventListener('message', listener)
  window.BusymateMicrophone.postMessage(JSON.stringify({ type: 'busymate.microphone.v1.request', id: 'mic-3', source: 'voice' }))
  await tick(); await tick()
  assert.equal(seen.length, 2)
})

test('a page listener that throws does not break delivery', async () => {
  const { window } = run({ invoke: async () => '{"id":"m","granted":false}' })
  const seen = []
  window.BusymateMicrophone.addEventListener('message', () => { throw new Error('page bug') })
  window.BusymateMicrophone.addEventListener('message', (event) => seen.push(event.data))
  window.BusymateMicrophone.postMessage('{"type":"busymate.microphone.v1.request","id":"m"}')
  await tick(); await tick()
  assert.equal(seen.length, 1)
})

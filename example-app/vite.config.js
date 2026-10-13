import { defineConfig } from 'vite'

// The SDK's JS bindings are linked from ../plugin/guest-js; resolve their
// @tauri-apps/api import from this app so there is exactly one copy.
export default defineConfig({
  clearScreen: false,
  resolve: { dedupe: ['@tauri-apps/api'] },
  server: { port: 1420, strictPort: true },
  build: { outDir: 'dist', target: 'es2021', emptyOutDir: true },
})

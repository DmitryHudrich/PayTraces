import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'node:path'
import tailwindcss from '@tailwindcss/vite'

// The C# public API (Ledgerscope.Accounts) runs on :5107 and ships no CORS
// policy, so the browser must talk to it same-origin through this proxy.
const apiTarget = process.env.VITE_PROXY_TARGET ?? 'http://localhost:5107'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@/app': resolve(__dirname, 'src/app'),
      '@/pages': resolve(__dirname, 'src/pages'),
      '@/shared': resolve(__dirname, 'src/shared'),
      graphology: resolve(__dirname, 'node_modules/graphology/dist/graphology.cjs.js'),
    },
  },
  server: {
    proxy: {
      // REST: strip the `/api` prefix the frontend adds; the service serves
      // endpoints at the root (`/auth/login`, `/cases`, ...).
      '/api': {
        target: apiTarget,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
      // SignalR graph hub — negotiate over http then upgrade to websocket.
      '/hubs': {
        target: apiTarget,
        changeOrigin: true,
        ws: true,
      },
    },
  },
})

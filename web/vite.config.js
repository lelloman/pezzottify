import { fileURLToPath, URL } from 'node:url'
import { readFileSync } from 'node:fs'
import { execSync } from 'node:child_process'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueJsx from '@vitejs/plugin-vue-jsx'
import vueDevTools from 'vite-plugin-vue-devtools'

// Read base version from VERSION file at repo root
function getBaseVersion() {
  try {
    return readFileSync('../VERSION', 'utf-8').trim()
  } catch {
    return '0.0'
  }
}

// Get commit count for patch version
function getCommitCount() {
  try {
    return execSync('git rev-list --count HEAD', { encoding: 'utf-8' }).trim()
  } catch {
    return '0'
  }
}

// Compute full version: MAJOR.MINOR.COMMIT-COUNT
const baseVersion = getBaseVersion()
const commitCount = getCommitCount()
const appVersion = `${baseVersion}.${commitCount}`

// https://vite.dev/config/
export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  plugins: [
    vue(),
    vueJsx(),
    vueDevTools(),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
  },
  server: {
    proxy: {
      '/v1': {
        target: 'http://localhost:3001',  // Your backend server
        changeOrigin: true,
        ws: true,  // Enable WebSocket proxying
        rewrite: (path) => path.replace(/^\/v1/, '/v1')
      }
    }
  }
})

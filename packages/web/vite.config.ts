import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'fs'
import { resolve } from 'path'

// Read server port from elore.toml if it exists, otherwise default to 3000.
function getApiTarget(): string {
  try {
    const content = readFileSync(resolve(__dirname, '../elore.toml'), 'utf-8')
    const hostMatch = content.match(/^\s*host\s*=\s*"([^"]+)"/m)
    const portMatch = content.match(/^\s*port\s*=\s*(\d+)/m)
    const host = hostMatch?.[1] ?? '127.0.0.1'
    const port = portMatch?.[1] ?? '3000'
    return `http://${host}:${port}`
  } catch {
    return 'http://127.0.0.1:3000'
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/api': getApiTarget(),
    },
  },
})

import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { execSync } from 'child_process'

// Get version info at build time
function getBuildVersion() {
  try {
    const gitHash = execSync('git rev-parse --short HEAD').toString().trim()
    const gitDate = execSync('git log -1 --format=%cd --date=short').toString().trim()
    return `${gitDate}-${gitHash}`
  } catch {
    return 'dev'
  }
}

export default defineConfig({
  plugins: [react()],
  define: {
    __BUILD_VERSION__: JSON.stringify(getBuildVersion()),
    __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    // Vite 8 / Rolldown dropped object-form manualChunks; codeSplitting is the replacement.
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: 'vendor-react',
              test: /[\\/]node_modules[\\/](?:react|react-dom)(?:[\\/]|$)/,
            },
            {
              name: 'vendor-charts',
              test: /[\\/]node_modules[\\/]recharts(?:[\\/]|$)/,
            },
            {
              name: 'vendor-icons',
              test: /[\\/]node_modules[\\/]lucide-react(?:[\\/]|$)/,
            },
          ],
        },
      },
    },
  },
})

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    open: true,
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
    // Cloudflare Pages expects _routes.json in dist for SPA fallback
    rollupOptions: {
      output: {
        manualChunks: undefined,
      },
    },
  },
});
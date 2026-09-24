import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    // Binds to 0.0.0.0 so an iPhone on the LAN can reach the PWA on the
    // same network as the dev server (Phase 5 iOS acceptance). Mirrors
    // web-ui's host:true; port differs so both clients can run at once.
    host: true,
  },
  build: {
    target: 'es2022',
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks: {
          react: ['react', 'react-dom', 'react-router-dom'],
          query: ['@tanstack/react-query', 'axios', 'zustand'],
        },
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './tests/setup.ts',
    css: true,
    passWithNoTests: true,
    coverage: { provider: 'v8', reporter: ['text', 'json-summary'] },
  },
});
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    // Binds to 0.0.0.0 (all interfaces) instead of Vite's localhost-only
    // default, so other devices on the LAN can reach the dev server at
    // http://<this-machine's-LAN-IP>:5173. Matches the same LAN-reachability
    // change made to api-server's ONYX_BIND (see start-backend.ps1's
    // -Bind 0.0.0.0:3000 option) — both need this, independently, since
    // Vite's dev server and api-server bind their sockets separately.
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
    // `provider` became required by vitest 1.6's coverage types after this
    // config was written against an earlier 1.x. Surfaced by the audit
    // (finding M-04: web-ui has no lockfile, so `npm install` resolves
    // whatever 1.x is newest at install time rather than a pinned version).
    coverage: { provider: 'v8', reporter: ['text', 'json-summary'] },
  },
});

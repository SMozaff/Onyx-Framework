import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

// Same Tauri-recommended Vite settings as desktop-shell/ui's own
// vite.config.ts — see that file's comment for the full rationale.
// Only real difference: port 5174, not 5173, so both Tauri apps
// (desktop-shell and this Admin shell) can run their dev servers at the
// same time without a port collision — matches admin-shell's own
// tauri.conf.json devUrl.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5174,
    strictPort: true,
    watch: {
      ignored: ["**/target/**", "**/*.rs"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});

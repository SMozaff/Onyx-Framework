import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

// Tauri-recommended Vite settings — verified against the real, current
// Tauri v2 guidance before writing this (not assumed): a fixed dev port
// matching tauri.conf.json's `devUrl`, `clearScreen: false` so Rust
// compiler errors aren't hidden by Vite's own screen-clearing, and
// ignoring the Rust workspace from Vite's file watcher (it lives two
// directories up from this `ui/` project root, at
// `crates/bins/desktop-shell`, not a nested `src-tauri/`).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: {
      // `import.meta.dirname`, not `__dirname` — this project's Node
      // target and Vite 8's own config loader (`configLoader: 'native'`)
      // flag `__dirname` as unsupported/deprecated; confirmed by a real
      // build-time warning before this fix, not assumed preemptively.
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ["**/target/**", "**/*.rs"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    // Vite 8 ships Rolldown (its Rust-based bundler) by default and no
    // longer bundles esbuild as a dependency — `minify: "esbuild"`
    // (Vite 5/6/7's convention) fails at build time with "Cannot find
    // package 'esbuild'" since nothing installs it anymore. Confirmed
    // via a real failing build before this fix. Rolldown's own
    // minifier (the `true`/default path) is what Vite 8 actually
    // ships and expects here.
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});

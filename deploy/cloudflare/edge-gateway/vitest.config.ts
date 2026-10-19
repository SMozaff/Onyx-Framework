import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const workerArgon2Mock = fileURLToPath(new URL("./test/argon2-wasm-edge.mock.ts", import.meta.url));
const wasmModuleMock = fileURLToPath(new URL("./test/wasm-module.mock.ts", import.meta.url));

export default defineConfig({
  resolve: {
    alias: [
      { find: /^argon2-wasm-edge\/wasm\/(?:argon2|blake2b)\.wasm$/, replacement: wasmModuleMock },
      { find: "argon2-wasm-edge", replacement: workerArgon2Mock },
    ],
  },
});

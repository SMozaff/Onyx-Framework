import { defineConfig } from "vitest/config";

// Separate from vite.config.ts on purpose: the production Vite build stays
// untouched, and this keeps the Admin shell's PKCE/auth regression tests on the
// same vitest convention the Worker and the other ONYX frontends already use.
export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    reporters: ["default"],
  },
});

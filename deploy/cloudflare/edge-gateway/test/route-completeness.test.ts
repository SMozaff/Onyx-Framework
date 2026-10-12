import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const read = (path: string) => readFileSync(root + "/" + path, "utf8");

describe("Admin API route completeness", () => {
  const settings = read("crates/bins/admin-shell/ui/src/pages/Settings.tsx");
  const users = read("crates/bins/admin-shell/ui/src/pages/Users.tsx");
  const worker = read("deploy/cloudflare/edge-gateway/src/index.ts");

  it("keeps every required Admin Settings endpoint registered with its expected method", () => {
    const required = [
      ["GET", "/api/admin/mobile-access"],
      ["PUT", "/api/admin/mobile-access"],
      ["POST", "/api/admin/policies"],
      ["POST", "/api/admin/legal-holds"],
    ] as const;
    for (const [method, path] of required) {
      expect(settings, "Admin Settings must call " + method + " " + path).toContain(path);
      expect(worker, "Worker must implement " + method + " " + path)
        .toContain('url.pathname==="' + path + '"&&request.method==="' + method + '"');
    }
  });

  it("implements the class-assignment endpoint used by the Admin Users screen", () => {
    expect(users).toContain("/api/admin/users/${user.id}/class");
    expect(worker).toContain("^\\/api\\/admin\\/users\\/[^/]+\\/class$");
    expect(worker).toContain("async function assignUserClass");
  });

  it("declares the canonical hierarchy handler as a JSON-array response and preserves client parser contracts", () => {
    expect(worker).toContain("return json(canonicalHierarchy(rows.results??[]));");
    expect(worker).not.toContain("return json({users:rows.results??[]});");
    expect(read("crates/applications/client-composition/src/hierarchy_cache.rs"))
      .toContain("let wire: Vec<HierarchyUserWire>");
    expect(read("mobile-android/app/src/main/kotlin/com/onyx/net/AuthApi.kt"))
      .toContain("Fetches the org's reporting-line tree");
  });
});

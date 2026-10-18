import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const read = (path: string) => readFileSync(root + "/" + path, "utf8");

type Route = { method: string; pattern: string };
type RequiredRoute = { method: string; path: string; source: string };

const settings = read("crates/bins/admin-shell/ui/src/pages/Settings.tsx");
const users = read("crates/bins/admin-shell/ui/src/pages/Users.tsx");
const worker = read("deploy/cloudflare/edge-gateway/src/index.ts");

const REQUIRED_ROUTES: RequiredRoute[] = [
  { method: "GET", path: "/api/admin/mobile-access", source: "Settings.tsx MobileAccessPanel" },
  { method: "PUT", path: "/api/admin/mobile-access", source: "Settings.tsx MobileAccessPanel" },
  { method: "POST", path: "/api/admin/policies", source: "Settings.tsx CreatePolicyForm" },
  { method: "POST", path: "/api/admin/legal-holds", source: "Settings.tsx LegalHoldPanel" },
  { method: "POST", path: "/api/command", source: "Settings.tsx PolicyPanel" },
  { method: "GET", path: "/api/admin/users", source: "Admin Users screen" },
  { method: "GET", path: "/api/allfather/organizations", source: "Organizations.tsx" },
  { method: "POST", path: "/api/allfather/organizations", source: "Organizations.tsx" },
  { method: "PATCH", path: "/api/allfather/organizations/:id", source: "Organizations.tsx" },
  { method: "POST", path: "/api/allfather/organizations/:id/activate", source: "Organizations.tsx" },
  { method: "POST", path: "/api/allfather/organizations/:id/deactivate", source: "Organizations.tsx" },
  { method: "GET", path: "/api/allfather/admins", source: "Organizations.tsx" },
  { method: "POST", path: "/api/allfather/admins", source: "Organizations.tsx" },
  { method: "POST", path: "/api/allfather/admins/:id/reactivate", source: "Organizations.tsx" },
  { method: "POST", path: "/api/admin/users", source: "Admin Users create screen" },
  { method: "POST", path: "/api/admin/users/:id/class", source: "Admin Users screen" },
  { method: "POST", path: "/api/admin/users/:id/password", source: "Admin Users screen" },
  { method: "POST", path: "/api/admin/users/:id/parent", source: "Admin Users screen" },
  { method: "PUT", path: "/api/admin/profiles", source: "Profiles.tsx" },
  { method: "GET", path: "/api/profiles", source: "Profiles.tsx" },
  { method: "GET", path: "/api/users/hierarchy", source: "Rust/Android hierarchy contract" },
  { method: "POST", path: "/api/relay-ticket", source: "Staff relay client" },
  { method: "POST", path: "/api/auth/login", source: "Admin/Staff login" },
  { method: "POST", path: "/api/auth/refresh", source: "HTTP client interceptor" },
  { method: "POST", path: "/api/auth/logout", source: "All shells logout" },
  { method: "GET", path: "/api/query", source: "useQuery hook" },
  { method: "POST", path: "/api/todo/lists", source: "Staff todo creation" },
  { method: "GET", path: "/api/events", source: "Realtime client" },
  { method: "GET", path: "/api/relay/:target_id", source: "Relay WebSocket client" },
];

function canonicalizeRegexPath(raw: string): string {
  return raw
    .replace(/^\/\^/, "")
    .replace(/\$\/$/, "")
    .replace(/\\\//g, "/")
    .replace(/\[\^\/\]\+/g, ":param");
}

function extractImplementedRoutes(source: string): Route[] {
  const dispatchStart = source.indexOf("async function dispatch(");
  const failClosed = source.indexOf('return json({error:"ROUTE_NOT_MIGRATED"', dispatchStart);
  if (dispatchStart < 0 || failClosed < 0) {
    throw new Error("Could not isolate dispatch() and its fail-closed fallback.");
  }

  const dispatch = source.slice(dispatchStart, failClosed);
  const routes: Route[] = [];
  for (const line of dispatch.split("\n")) {
    const methodMatch = line.match(/request\.method\s*===\s*("(?:\\.|[^"\\])*")/);
    if (!methodMatch) continue;
    const method = JSON.parse(methodMatch[1]) as string;

    const exactMatch = line.match(/url\.pathname\s*===\s*("(?:\\.|[^"\\])*")/);
    if (exactMatch) {
      routes.push({ method, pattern: JSON.parse(exactMatch[1]) as string });
      continue;
    }

    const prefixMatch = line.match(/url\.pathname\.startsWith\(\s*("(?:\\.|[^"\\])*")\s*\)/);
    if (prefixMatch) {
      routes.push({ method, pattern: (JSON.parse(prefixMatch[1]) as string) + "*" });
      continue;
    }

    const regexMatch = line.match(/(\/\^.*?\$\/)\.test\(url\.pathname\)/);
    if (regexMatch) {
      routes.push({ method, pattern: canonicalizeRegexPath(regexMatch[1]) });
    }
  }
  return routes;
}

function routeMatches(requiredPath: string, implementedPattern: string): boolean {
  if (implementedPattern.endsWith("*")) {
    return requiredPath.startsWith(implementedPattern.slice(0, -1));
  }
  const required = requiredPath.split("/").filter(Boolean);
  const implemented = implementedPattern.split("/").filter(Boolean);
  if (required.length !== implemented.length) return false;
  return required.every((segment, index) => {
    const actual = implemented[index];
    return segment === actual || (segment.startsWith(":") && actual.startsWith(":"));
  });
}

describe("Admin API route completeness", () => {
  it("keeps every required Admin Settings endpoint registered with its expected method", () => {
    for (const [method, path] of [
      ["GET", "/api/admin/mobile-access"],
      ["PUT", "/api/admin/mobile-access"],
      ["POST", "/api/admin/policies"],
      ["POST", "/api/admin/legal-holds"],
    ] as const) {
      expect(settings, "Admin Settings must call " + method + " " + path).toContain(path);
    }
  });

  it("registers the tenant-scoped Admin users list and create endpoints", () => {
    expect(users).toContain('apiClient.get<UserRow[]>("/api/admin/users")');
    expect(worker).toContain('url.pathname==="/api/admin/users"&&request.method==="GET"');
    expect(worker).toContain('url.pathname==="/api/admin/users"&&request.method==="POST"');
    expect(worker).toContain("async function listAdminUsers");
  });

  it("implements the class-assignment endpoint used by the Admin Users screen", () => {
    expect(users).toContain("/api/admin/users/${user.id}/class");
    expect(worker).toContain("^\\/api\\/admin\\/users\\/[^/]+\\/class$");
    expect(worker).toContain("async function assignUserClass");
  });

  it("defaults legal-hold creation to a supported UUID-backed aggregate, not a hash-keyed file asset", () => {
    expect(settings).toContain('useState("task")');
    expect(settings).not.toContain('useState("file_asset")');
  });

  it("preserves the canonical hierarchy JSON-array response and client parser contracts", () => {
    expect(worker).toContain("return json(canonicalHierarchy(rows.results??[]));");
    expect(worker).not.toContain("return json({users:rows.results??[]});");
    expect(read("crates/applications/client-composition/src/hierarchy_cache.rs"))
      .toContain("let wire: Vec<HierarchyUserWire>");
    expect(read("mobile-android/app/src/main/kotlin/com/onyx/net/AuthApi.kt"))
      .toContain("Fetches the org's reporting-line tree");
  });

  it("matches the current ONYX-managed All-Father credential contract", () => {
    const implemented = extractImplementedRoutes(worker);
    expect(implemented).toContainEqual({ method: "POST", pattern: "/api/auth/login" });
    expect(implemented.some((route) => route.method === "POST" && route.pattern === "/api/auth/supabase")).toBe(false);
  });

  it("extracts representative routes from dispatch()", () => {
    const implemented = extractImplementedRoutes(worker);
    expect(implemented).toContainEqual({ method: "POST", pattern: "/api/auth/login" });
    expect(implemented).toContainEqual({ method: "POST", pattern: "/api/command" });
    expect(implemented).toContainEqual({ method: "GET", pattern: "/api/query" });
  });

  for (const required of REQUIRED_ROUTES) {
    it(`${required.method} ${required.path} exists for ${required.source}`, () => {
      const found = extractImplementedRoutes(worker).some(
        (route) => route.method === required.method && routeMatches(required.path, route.pattern),
      );
      expect(
        found,
        `Missing ${required.method} ${required.path} required by ${required.source}. Implement it or remove the client call.`,
      ).toBe(true);
    });
  }

  it("rejects wildcard CORS origins on credential-capable responses", () => {
    const applyCorsStart = worker.indexOf("function applyCors(");
    const exportStart = worker.indexOf("export default", applyCorsStart);
    const applyCors = worker.slice(applyCorsStart, exportStart);
    expect(worker).toContain('allowed.includes(origin)&&!allowed.includes("*")');
    expect(applyCors).toContain('headers.delete("access-control-allow-origin")');
    expect(applyCors).toContain('headers.delete("access-control-allow-credentials")');
    expect(applyCors).not.toMatch(/headers\.set\("access-control-allow-origin",\s*"\*"\)/);
  });

  it("fails closed for unknown paths", () => {
    expect(worker).toContain("ROUTE_NOT_MIGRATED");
    expect(worker).toContain("FEATURE_UNAVAILABLE");
  });

  it("never uses the mobile_observer label as an authorization gate", () => {
    expect(worker).not.toContain('session.client_type==="mobile_observer"');
    expect(worker).not.toContain('session.client_type!=="mobile_observer"');
    expect(worker.match(/mobileCapabilityGranted\(env,session\)/g) ?? []).toHaveLength(2);
  });
});

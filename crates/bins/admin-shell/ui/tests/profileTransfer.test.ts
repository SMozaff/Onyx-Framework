import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

const session = new Map<string, string>();
Object.defineProperty(globalThis, "sessionStorage", {
  configurable: true,
  value: {
    getItem: (key: string) => session.get(key) ?? null,
    setItem: (key: string, value: string) => void session.set(key, value),
    removeItem: (key: string) => void session.delete(key),
    clear: () => session.clear(),
  },
});

let apiClient: (typeof import("../src/api/client"))["apiClient"];
let useAuthStore: (typeof import("../src/stores/authStore"))["useAuthStore"];
let transfer: typeof import("../src/pages/profileTransfer");
let originalAdapter: unknown;

beforeAll(async () => {
  ({ apiClient } = await import("../src/api/client"));
  ({ useAuthStore } = await import("../src/stores/authStore"));
  transfer = await import("../src/pages/profileTransfer");
  originalAdapter = apiClient.defaults.adapter;
});

afterEach(() => {
  apiClient.defaults.adapter = originalAdapter as never;
  useAuthStore.setState({ accessToken: null, refreshToken: null, user: null, isAuthenticated: false });
  session.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function authenticate() {
  useAuthStore.setState({ accessToken: "admin-access-token" });
}

describe("profile batch transfer requests", () => {
  it.each([
    ["staff.csv", "text/csv", "user_id,full_name\naccount-1,CSV Person\n"],
    ["staff.json", "application/json", '[{"user_id":"account-1","full_name":"JSON Person"}]'],
  ])("posts %s as an exact raw body with %s", async (name, contentType, contents) => {
    authenticate();
    const adapter = vi.fn(async (config: unknown) => ({
      data: { total_rows: 1, created: 1, updated: 0, failed: 0, results: [] },
      status: 200,
      statusText: "OK",
      headers: {},
      config,
    }));
    apiClient.defaults.adapter = adapter as never;

    await transfer.importProfileFile({
      name,
      type: contentType,
      text: async () => contents,
    } as File);

    const request = adapter.mock.calls[0]?.[0] as {
      url: string;
      data: string;
      headers: { get: (name: string) => string | undefined };
    };
    expect(request.url).toBe("/api/admin/profiles/import");
    expect(request.data).toBe(contents);
    expect(request.headers.get("Content-Type")).toBe(contentType);
    expect(request.headers.get("Authorization")).toBe("Bearer admin-access-token");
  });

  it("gets an authenticated export as a Blob and saves the expected filename", async () => {
    authenticate();
    const exported = new Blob(["user_id,full_name\naccount-1,Exported Person\n"], { type: "text/csv" });
    const adapter = vi.fn(async (config: unknown) => ({
      data: exported,
      status: 200,
      statusText: "OK",
      headers: { "content-type": "text/csv" },
      config,
    }));
    const saveBlob = vi.fn();
    apiClient.defaults.adapter = adapter as never;

    await transfer.exportProfileFile("csv", { saveBlob });

    const request = adapter.mock.calls[0]?.[0] as {
      url: string;
      responseType: string;
      headers: { get: (name: string) => string | undefined };
    };
    expect(request.url).toBe("/api/admin/profiles/export?format=csv");
    expect(request.responseType).toBe("blob");
    expect(request.headers.get("Authorization")).toBe("Bearer admin-access-token");
    expect(saveBlob).toHaveBeenCalledWith(exported, "staff-profiles.csv");
  });

  it("releases the object URL even when a browser download click fails", () => {
    const remove = vi.fn();
    const click = vi.fn(() => { throw new Error("download blocked"); });
    const link = { href: "", download: "", style: { display: "" }, remove, click };
    const appendChild = vi.fn();
    vi.stubGlobal("document", {
      body: { appendChild },
      createElement: vi.fn(() => link),
    });
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:test-export");
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL");

    expect(() => transfer.saveProfileExportBlob(new Blob(["data"]), "staff-profiles.csv"))
      .toThrow("download blocked");
    expect(appendChild).toHaveBeenCalledWith(link);
    expect(remove).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:test-export");
  });
});

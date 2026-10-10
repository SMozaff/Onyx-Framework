type Json = Record<string, unknown>;

type SupabaseJwt = Json & {
  sub: string;
  email?: string;
  email_verified?: boolean;
  exp: number;
  iss: string;
};

function b64uDecode(value: string): Uint8Array {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (value.length % 4)) % 4);
  const binary = atob(normalized);
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

function decodePart(value: string): Json {
  return JSON.parse(new TextDecoder().decode(b64uDecode(value))) as Json;
}

function supabaseBase(env: { SUPABASE_URL?: string }): string {
  const url = (env.SUPABASE_URL ?? "").replace(/\/$/, "");
  if (!url) throw new Error("SUPABASE_URL is not configured");
  return url;
}

function publicKey(env: { SUPABASE_PUBLISHABLE_KEY?: string }): string {
  if (!env.SUPABASE_PUBLISHABLE_KEY) throw new Error("SUPABASE_PUBLISHABLE_KEY is not configured");
  return env.SUPABASE_PUBLISHABLE_KEY;
}

function secretKey(env: { SUPABASE_SECRET_KEY?: string }): string {
  if (!env.SUPABASE_SECRET_KEY) throw new Error("SUPABASE_SECRET_KEY is not configured");
  return env.SUPABASE_SECRET_KEY;
}

export async function verifySupabaseJwt(env: { SUPABASE_URL?: string; SUPABASE_JWKS_URL?: string }, token: string): Promise<SupabaseJwt> {
  const parts = token.split(".");
  if (parts.length !== 3) throw new Error("invalid supabase token");

  const header = decodePart(parts[0]);
  const payload = decodePart(parts[1]) as SupabaseJwt;
  const alg = String(header.alg ?? "");
  const kid = typeof header.kid === "string" ? header.kid : "";
  if (!kid || (alg !== "RS256" && alg !== "ES256")) throw new Error("unsupported supabase jwt");

  const base = supabaseBase(env);
  const expectedIssuer = `${base}/auth/v1`;
  if (payload.iss !== expectedIssuer || !payload.sub || Number(payload.exp) <= Math.floor(Date.now() / 1000)) {
    throw new Error("invalid supabase claims");
  }

  const jwksUrl = env.SUPABASE_JWKS_URL || `${base}/auth/v1/.well-known/jwks.json`;
  const response = await fetch(jwksUrl, { headers: { accept: "application/json" } });
  if (!response.ok) throw new Error("supabase jwks unavailable");
  const jwks = await response.json() as { keys?: Json[] };
  const jwk = jwks.keys?.find((key) => key.kid === kid);
  if (!jwk) throw new Error("supabase signing key not found");

  const algorithm = alg === "RS256"
    ? { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }
    : { name: "ECDSA", namedCurve: "P-256" };
  const verifyAlgorithm = alg === "RS256"
    ? { name: "RSASSA-PKCS1-v1_5" }
    : { name: "ECDSA", hash: "SHA-256" };

  const key = await crypto.subtle.importKey("jwk", jwk as JsonWebKey, algorithm, false, ["verify"]);
  const verified = await crypto.subtle.verify(
    verifyAlgorithm,
    key,
    b64uDecode(parts[2]),
    new TextEncoder().encode(`${parts[0]}.${parts[1]}`),
  );
  if (!verified) throw new Error("invalid supabase signature");
  return payload;
}

export async function getSupabaseUser(
  env: { SUPABASE_URL?: string; SUPABASE_PUBLISHABLE_KEY?: string },
  accessToken: string,
): Promise<{ id: string; email: string | null; email_confirmed_at: string | null }> {
  const response = await fetch(`${supabaseBase(env)}/auth/v1/user`, {
    method: "GET",
    headers: {
      apikey: publicKey(env),
      authorization: `Bearer ${accessToken}`,
      accept: "application/json",
    },
  });
  const body = await response.json().catch(() => ({})) as Json;
  if (!response.ok || typeof body.id !== "string") {
    throw new Error("SUPABASE_USER_VERIFICATION_FAILED");
  }
  return {
    id: body.id,
    email: typeof body.email === "string" ? body.email : null,
    email_confirmed_at: typeof body.email_confirmed_at === "string"
      ? body.email_confirmed_at
      : typeof body.confirmed_at === "string" ? body.confirmed_at : null,
  };
}

export async function passwordSignIn(
  env: { SUPABASE_URL?: string; SUPABASE_PUBLISHABLE_KEY?: string },
  email: string,
  password: string,
): Promise<{ access_token: string; refresh_token: string; user: Json }> {
  const response = await fetch(`${supabaseBase(env)}/auth/v1/token?grant_type=password`, {
    method: "POST",
    headers: {
      apikey: publicKey(env),
      authorization: `Bearer ${publicKey(env)}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ email, password }),
  });
  const body = await response.json().catch(() => ({})) as Json;
  if (!response.ok || typeof body.access_token !== "string" || typeof body.refresh_token !== "string") {
    throw new Error("SUPABASE_INVALID_CREDENTIALS");
  }
  return { access_token: body.access_token, refresh_token: body.refresh_token, user: (body.user as Json) ?? {} };
}

async function adminRequest(
  env: { SUPABASE_URL?: string; SUPABASE_SECRET_KEY?: string },
  path: string,
  method: string,
  body?: Json,
): Promise<Json> {
  const response = await fetch(`${supabaseBase(env)}${path}`, {
    method,
    headers: {
      apikey: secretKey(env),
      authorization: `Bearer ${secretKey(env)}`,
      "content-type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const result = await response.json().catch(() => ({})) as Json;
  if (!response.ok) {
    const message = typeof result.msg === "string" ? result.msg : typeof result.message === "string" ? result.message : "Supabase admin request failed";
    throw new Error(message);
  }
  return result;
}

export async function adminCreateUser(
  env: { SUPABASE_URL?: string; SUPABASE_SECRET_KEY?: string },
  email: string,
  password: string,
  metadata: Json,
): Promise<string> {
  const user = await adminRequest(env, "/auth/v1/admin/users", "POST", {
    email,
    password,
    email_confirm: true,
    user_metadata: metadata,
  });
  if (typeof user.id !== "string") throw new Error("SUPABASE_USER_ID_MISSING");
  return user.id;
}

export async function adminSetUserBanned(
  env: { SUPABASE_URL?: string; SUPABASE_SECRET_KEY?: string },
  userId: string,
  banned: boolean,
): Promise<void> {
  await adminRequest(env, `/auth/v1/admin/users/${encodeURIComponent(userId)}`, "PUT", {
    ban_duration: banned ? "876000h" : "none",
  });
}

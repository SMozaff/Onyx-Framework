interface Env {
  DB?: D1Database;
}

const JSON_HEADERS = {
  "content-type": "application/json; charset=utf-8",
  "cache-control": "no-store",
};

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: JSON_HEADERS,
  });
}

async function ready(env: Env): Promise<Response> {
  if (!env.DB) {
    return json(
      {
        status: "degraded",
        service: "onyx-cloudflare-worker",
        database: "not-configured",
        migration: "d1-binding-required",
      },
      503,
    );
  }

  try {
    await env.DB.prepare("SELECT 1 AS ok").first();
    return json({
      status: "ok",
      service: "onyx-cloudflare-worker",
      database: "d1",
    });
  } catch {
    return json(
      {
        status: "degraded",
        service: "onyx-cloudflare-worker",
        database: "unavailable",
      },
      503,
    );
  }
}

function migrationPending(pathname: string): Response {
  return json(
    {
      error: "API_MIGRATION_IN_PROGRESS",
      route: pathname,
      message:
        "This ONYX route is not yet ported from Axum to the Cloudflare Worker. The legacy Axum API is intentionally not used as an origin.",
    },
    501,
  );
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") {
      return new Response(null, {
        status: 204,
        headers: {
          "access-control-allow-origin": "*",
          "access-control-allow-methods": "GET,POST,PUT,DELETE,OPTIONS",
          "access-control-allow-headers": "Authorization,Content-Type",
          "access-control-max-age": "86400",
        },
      });
    }

    if (url.pathname === "/health") {
      return json({ status: "ok", service: "onyx-cloudflare-worker" });
    }

    if (url.pathname === "/ready") {
      return ready(env);
    }

    // Cloudflare is now the application boundary. There is deliberately no
    // ONYX_ORIGIN_URL and no fetch() to Render or another container host.
    // Routes are ported incrementally, with D1 providing the free SQLite
    // persistence layer.
    return migrationPending(url.pathname);
  },
};

interface Env {
  ONYX_ORIGIN_URL: string;
}

const HOP_BY_HOP_HEADERS = new Set([
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
]);

function originRequest(request: Request, origin: URL): Request {
  const headers = new Headers(request.headers);
  headers.delete("host");

  return new Request(origin, {
    method: request.method,
    headers,
    body: request.method === "GET" || request.method === "HEAD" ? undefined : request.body,
    redirect: "manual",
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const origin = new URL(env.ONYX_ORIGIN_URL);
    const incoming = new URL(request.url);

    if (origin.protocol !== "https:") {
      return new Response("ONYX_ORIGIN_URL must use https", { status: 500 });
    }

    origin.pathname = incoming.pathname;
    origin.search = incoming.search;

    const upstream = await fetch(originRequest(request, origin));
    const headers = new Headers(upstream.headers);

    for (const header of HOP_BY_HOP_HEADERS) {
      headers.delete(header);
    }

    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers,
    });
  },
};

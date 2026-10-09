import { Container } from "@cloudflare/containers";

interface Env {
  ONYX_API: DurableObjectNamespace<OnyxApiContainer>;
  DATABASE_URL: string;
  ONYX_GOVERNANCE_DATABASE_URL: string;
  ONYX_AUTHORITY_SIGNING_KEY: string;
  ONYX_CORS_ALLOWED_ORIGINS: string;
  CLERK_ISSUER: string;
  CLERK_SECRET_KEY: string;
  CLERK_JWKS_URL: string;
  ONYX_BLOB_STORE_S3_ACCESS_KEY_ID: string;
  ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY: string;
}

const CONTAINER_ENV_KEYS = [
  "DATABASE_URL",
  "ONYX_GOVERNANCE_DATABASE_URL",
  "ONYX_AUTHORITY_SIGNING_KEY",
  "ONYX_CORS_ALLOWED_ORIGINS",
  "CLERK_ISSUER",
  "CLERK_SECRET_KEY",
  "CLERK_JWKS_URL",
  "ONYX_BLOB_STORE_S3_ACCESS_KEY_ID",
  "ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY",
] as const;

type ContainerEnv = Record<string, string>;

export class OnyxApiContainer extends Container {
  defaultPort = 10000;
  sleepAfter = "30m";
  enableInternet = true;
  pingEndpoint = "container/ready";

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);

    const containerEnv: ContainerEnv = {
      ONYX_ENV: "production",
      ONYX_BIND: "0.0.0.0:10000",
      ONYX_METRICS_BIND: "127.0.0.1:9090",
      RUST_LOG: "info",
      ONYX_BLOB_STORE_BACKEND: "huggingface",
      ONYX_BLOB_STORE_S3_ENDPOINT: "https://s3.hf.co/Arronthemalkavian",
      ONYX_BLOB_STORE_S3_BUCKET: "onyx",
    };

    for (const key of CONTAINER_ENV_KEYS) {
      const value = env[key];
      if (value) {
        containerEnv[key] = value;
      }
    }

    this.envVars = containerEnv;
  }

  override onError(error: unknown): void {
    console.error("ONYX API container error", error);
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    return env.ONYX_API.getByName("production").fetch(request);
  },
} satisfies ExportedHandler<Env>;

import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { apiClient } from "@/api/client";
import ConnectionSettings from "@/components/ConnectionSettings";
import { LanguageSwitcher } from "@/i18n/LanguageSwitcher";
import { useAuthStore } from "@/stores/authStore";
import { getServerAddress } from "@/utils/serverAddress";
import { loadClerk } from "@/auth/clerk";

export default function Login() {
  const navigate = useNavigate();
  const signInRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [showServerSettings, setShowServerSettings] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let cleanup: (() => void) | undefined;

    void loadClerk()
      .then((clerk) => {
        if (cancelled || !signInRef.current) return;
        clerk.mountSignIn(signInRef.current, {
          appearance: { elements: { rootBox: "w-full" } },
        });

        const exchange = async (session: { getToken: () => Promise<string | null> } | null) => {
          if (!session || cancelled) return;
          setLoading(true);
          setError(null);
          try {
            const token = await session.getToken();
            if (!token) throw new Error("Clerk did not provide a session token.");
            const response = await apiClient.post("/api/auth/clerk", null, {
              headers: { Authorization: `Bearer ${token}` },
            });
            useAuthStore.getState().login(response.data);
            navigate("/", { replace: true });
          } catch (err) {
            const status = (err as { response?: { status?: number } }).response?.status;
            if (status === 403) {
              setError("Your Google/Clerk identity is authenticated but is not provisioned for ONYX authority.");
            } else if (status === 401) {
              setError("The Clerk session could not be verified by ONYX.");
            } else {
              setError("Could not exchange the Clerk session with the ONYX API.");
            }
            setShowServerSettings(false);
          } finally {
            if (!cancelled) setLoading(false);
          }
        };

        cleanup = clerk.addListener(({ session }) => void exchange(session));
        void exchange(clerk.session);
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setError(err.message);
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
      cleanup?.();
      if (signInRef.current) {
        void loadClerk().then((clerk) => {
          if (signInRef.current) clerk.unmountSignIn(signInRef.current);
        });
      }
    };
  }, [navigate]);

  return (
    <div className="onyx-auth-shell flex flex-col lg:flex-row">
      <section className="onyx-auth-aside">
        <div className="relative z-10 flex items-center gap-2.5">
          <span className="onyx-brand-mark" aria-hidden="true">O</span>
          <div>
            <p className="text-[0.72rem] font-extrabold tracking-[0.24em] text-white">ONYX</p>
            <p className="mt-0.5 text-[0.62rem] text-sky-100/70">Administration</p>
          </div>
        </div>
        <div className="onyx-auth-copy">
          <p className="text-[0.72rem] font-extrabold tracking-[0.19em] text-sky-100/90">SECURE ACCESS</p>
          <h2 className="mt-4 max-w-md text-4xl font-light leading-[1.03] tracking-[-0.045em] text-white sm:text-5xl">
            Sign in with Google through Clerk.
          </h2>
          <p className="mt-5 max-w-lg text-sm leading-6 text-sky-50/85">
            ONYX verifies your Clerk session and then applies ONYX authorization. The All-Father
            authority is reserved for the verified designated identity.
          </p>
        </div>
      </section>

      <section className="flex flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:px-16">
        <div className="onyx-auth-card w-full max-w-md p-6 sm:p-7">
          <div className="mb-5 flex items-center justify-between">
            <h1 className="text-lg font-semibold text-onyx-text">ONYX Admin</h1>
            <LanguageSwitcher />
          </div>

          {error && (
            <div role="alert" className="mb-4 rounded-md border border-onyx-border bg-onyx-surface-hover p-3 text-sm text-onyx-text">
              {error}
            </div>
          )}

          {loading && (
            <p className="mb-3 text-sm text-onyx-text-dim">Initializing secure Clerk authentication…</p>
          )}

          <div ref={signInRef} className="min-h-[360px]" />

          {showServerSettings && (
            <ConnectionSettings />
          )}

          <button
            type="button"
            className="mt-4 text-sm text-onyx-text-dim underline"
            onClick={() => setShowServerSettings((value) => !value)}
          >
            Server connection
          </button>
          <p className="mt-3 text-xs text-onyx-text-dim">
            Current API: {getServerAddress()}
          </p>
        </div>
      </section>
    </div>
  );
}

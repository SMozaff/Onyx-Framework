import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Navigate, Route, Routes } from "react-router-dom";
import MainLayout from "@/components/Layout/MainLayout";
import Dashboard from "@/pages/Dashboard";
import Missions from "@/pages/Missions";
import Tasks from "@/pages/Tasks";
import Approvals from "@/pages/Approvals";
import Messaging from "@/pages/Messaging";
import Files from "@/pages/Files";
import Notifications from "@/pages/Notifications";
import Login from "@/pages/Login";
import Settings from "@/pages/Settings";
import { SessionProvider, type SessionWire } from "@/hooks/useSession";
import { toUserFacingError, type UserFacingError } from "@/utils/userFacingError";

/**
 * Loads the native persisted session before exposing any operational route.
 * The desktop shell owns authenticated tokens in Rust secure storage, so the
 * React layer asks the real `get_current_session` command on startup rather
 * than creating local placeholder identities. A fresh install and a completed
 * logout intentionally receive `null` and are sent to the login screen.
 */
export default function App() {
  const [session, setSession] = useState<SessionWire | null | undefined>(undefined);
  const [startupError, setStartupError] = useState<UserFacingError | null>(null);
  const [retryToken, setRetryToken] = useState(0);
  const [loginServerAddress, setLoginServerAddress] = useState("http://127.0.0.1:3000");

  useEffect(() => {
    let cancelled = false;
    setStartupError(null);
    setSession(undefined);

    void invoke<SessionWire | null>("get_current_session")
      .then((storedSession) => {
        if (cancelled) return;
        setSession(storedSession);
        if (storedSession !== null) {
          setLoginServerAddress(storedSession.serverAddress);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setStartupError(toUserFacingError(error));
          setSession(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [retryToken]);

  const logout = useCallback(
    async (nextLoginAddress?: string) => {
      await invoke("logout");
      setLoginServerAddress(nextLoginAddress ?? session?.serverAddress ?? loginServerAddress);
      setSession(null);
    },
    [loginServerAddress, session],
  );

  if (session === undefined) return <LoadingSession />;
  if (startupError !== null) return <StartupError error={startupError} onRetry={() => setRetryToken((attempt) => attempt + 1)} />;
  if (session === null) {
    return <Login initialServerAddress={loginServerAddress} onAuthenticated={(authenticated) => {
      setLoginServerAddress(authenticated.serverAddress);
      setSession(authenticated);
    }} />;
  }

  return (
    <SessionProvider session={session}>
      <MainLayout onLogout={logout}>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/missions" element={<Missions />} />
          <Route path="/missions/:missionId" element={<Missions />} />
          <Route path="/tasks" element={<Tasks />} />
          <Route path="/tasks/:taskId" element={<Tasks />} />
          <Route path="/approvals" element={<Approvals />} />
          <Route path="/messaging" element={<Messaging />} />
          <Route path="/messaging/:conversationId" element={<Messaging />} />
          <Route path="/files" element={<Files />} />
          <Route path="/files/:fileAssetId" element={<Files />} />
          <Route path="/settings" element={<Settings onRequireReauthentication={logout} />} />
          <Route path="/notifications" element={<Notifications />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </MainLayout>
    </SessionProvider>
  );
}

function LoadingSession() {
  return <div className="flex min-h-screen items-center justify-center bg-onyx-bg px-4 text-sm text-onyx-text-dim">Loading secure session…</div>;
}

function StartupError({ error, onRetry }: { error: UserFacingError; onRetry: () => void }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-onyx-bg px-4">
      <section className="w-full max-w-md rounded-lg border border-onyx-border bg-onyx-surface p-6" role="alert">
        <h1 className="text-lg font-semibold text-onyx-text">{error.title}</h1>
        <p className="mt-2 text-sm text-onyx-text-dim">{error.message}</p>
        <button type="button" onClick={onRetry} className="mt-4 rounded-md bg-onyx-accent px-3 py-1.5 text-sm font-medium text-white">Retry</button>
      </section>
    </div>
  );
}

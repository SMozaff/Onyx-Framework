import { lazy, Suspense } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import MainLayout from "@/components/Layout/MainLayout";
import { useAuthStore } from "@/stores/authStore";

const LoginPage = lazy(() => import("@/pages/Login"));
const UsersPage = lazy(() => import("@/pages/Users"));
const SettingsPage = lazy(() => import("@/pages/Settings"));
const ProfilesPage = lazy(() => import("@/pages/Profiles"));

/**
 * Root layout + route table for the ONYX Admin platform. A deliberately
 * separate application from `desktop-shell` and `web-ui` — see
 * `DECISIONS.md`'s "Admin Platform" section for the product decision.
 *
 * Two gates, not one: `ProtectedLayout` requires a valid session (same
 * as `web-ui`'s own pattern); `AdminOnlyLayout` additionally requires
 * `is_admin` on the logged-in user (added to `/api/auth/login`'s
 * response specifically for this app — see
 * `security_application`/`api_server::routes::auth::LoginUser`'s own
 * doc comment). A non-admin who successfully authenticates still sees a
 * clear "you don't have admin access" message rather than a confusing
 * wall of 403s from every action — the server enforces the real
 * authority check regardless; this is purely a client-side UX
 * improvement, not a security boundary in itself.
 */
function ProtectedLayout() {
  const authenticated = useAuthStore((state) => state.isAuthenticated);
  return authenticated ? <AdminOnlyLayout /> : <Navigate to="/login" replace />;
}

function AdminOnlyLayout() {
  const user = useAuthStore((state) => state.user);
  if (!user?.is_admin) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-onyx-bg px-4">
        <div className="max-w-md rounded-lg border border-onyx-border bg-onyx-surface p-6 text-center">
          <h1 className="text-lg font-semibold text-onyx-text">Admin access required</h1>
          <p className="mt-2 text-sm text-onyx-text-dim">
            Your account ({user?.username}) does not have administrator access. Contact your
            organization's Admin if you believe this is incorrect.
          </p>
          <button
            type="button"
            onClick={() => useAuthStore.getState().logout()}
            className="mt-4 rounded-md bg-onyx-surface-hover px-3 py-1.5 text-sm text-onyx-text"
          >
            Log out
          </button>
        </div>
      </div>
    );
  }
  return <MainLayout />;
}

export default function App() {
  return (
    <Suspense
      fallback={
        <div className="flex min-h-screen items-center justify-center text-onyx-text-dim">
          Loading ONYX Admin…
        </div>
      }
    >
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route element={<ProtectedLayout />}>
          <Route index element={<UsersPage />} />
          <Route path="users" element={<UsersPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="settings/:policyId" element={<SettingsPage />} />
          <Route path="profiles" element={<ProfilesPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Suspense>
  );
}

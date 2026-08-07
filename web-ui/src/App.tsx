import { lazy, Suspense } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import MainLayout from './components/Layout/MainLayout';
import { useAuthStore } from './stores/authStore';

const LoginPage = lazy(() => import('./pages/Login'));
const DashboardPage = lazy(() => import('./pages/Dashboard'));
const MissionsPage = lazy(() => import('./pages/Missions'));
const TasksPage = lazy(() => import('./pages/Tasks'));
const NotificationsPage = lazy(() => import('./pages/Notifications'));
const ApprovalsPage = lazy(() => import('./pages/Approvals'));
const ReportsPage = lazy(() => import('./pages/Reports'));

function ProtectedLayout() {
  const authenticated = useAuthStore((state) => state.isAuthenticated);
  return authenticated ? <MainLayout /> : <Navigate to="/login" replace />;
}

export default function App() {
  return (
    <Suspense fallback={<div className="app-loading" role="status">Loading ONYX…</div>}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route element={<ProtectedLayout />}>
          <Route index element={<DashboardPage />} />
          <Route path="missions" element={<MissionsPage />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="notifications" element={<NotificationsPage />} />
          <Route path="approvals" element={<ApprovalsPage />} />
          <Route path="reports" element={<ReportsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Suspense>
  );
}

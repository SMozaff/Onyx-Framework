import { Navigate, useRoutes } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { LoginPage } from '../pages/Login';
import { DashboardPage } from '../pages/Dashboard';
import { MissionsPage } from '../pages/Missions';
import { MissionDetailPage } from '../pages/Missions/MissionDetail';
import { TasksPage } from '../pages/Tasks';
import { TaskDetailPage } from '../pages/Tasks/TaskDetail';
import { NotificationsPage } from '../pages/Notifications';
import { ApprovalsPage } from '../pages/Approvals';
import { ReportsPage } from '../pages/Reports';
import { FileDetailPage } from '../pages/Files/FileDetail';
import { ObserverLayout } from '../components/Layout';
import { NotFoundPage } from '../pages/NotFound';

function Protected({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated);
  if (!isAuthenticated) return <Navigate to="/login" replace />;
  return <>{children}</>;
}

export function AppRoutes() {
  return useRoutes([
    { path: '/login', element: <LoginPage /> },
    {
      element: (
        <Protected>
          <ObserverLayout />
        </Protected>
      ),
      children: [
        { index: true, element: <Navigate to="/dashboard" replace /> },
        { path: '/dashboard', element: <DashboardPage /> },
        { path: '/missions', element: <MissionsPage /> },
        { path: '/mission/:id', element: <MissionDetailPage /> },
        { path: '/tasks', element: <TasksPage /> },
        { path: '/task/:id', element: <TaskDetailPage /> },
        { path: '/notifications', element: <NotificationsPage /> },
        { path: '/approvals', element: <ApprovalsPage /> },
        { path: '/reports', element: <ReportsPage /> },
        // Phase 3.1: no file *listing* exists yet (api-server has no
        // FileAsset index). A hash-addressed file is still reachable.
        { path: '/files/:contentHash', element: <FileDetailPage /> },
        { path: '*', element: <NotFoundPage /> },
      ],
    },
  ]);
}
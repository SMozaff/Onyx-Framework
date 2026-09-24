import { Navigate, useRoutes } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { LoginPage } from '../pages/Login';
import { DashboardPage } from '../pages/Dashboard';
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
        { index: true, element: <DashboardPage /> },
        // Phase 2.3 fills in /missions, /missions/:id, /tasks, /tasks/:id,
        // /notifications, /approvals, /approvals/:id, /profile.
        { path: '*', element: <NotFoundPage /> },
      ],
    },
  ]);
}
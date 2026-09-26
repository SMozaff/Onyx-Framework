import { Link } from 'react-router-dom';

export function NotFoundPage() {
  return (
    <div className="py-12 text-center">
      <h2 className="text-lg font-semibold text-slate-900">Page not found</h2>
      <p className="mt-1 text-sm text-slate-500">
        This projection view is not part of the observer client surface.
      </p>
      <Link className="mt-4 inline-block text-sm underline underline-offset-4" to="/dashboard">
        Back to dashboard
      </Link>
    </div>
  );
}
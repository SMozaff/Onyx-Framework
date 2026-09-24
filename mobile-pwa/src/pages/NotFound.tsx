import { Link } from 'react-router-dom';

export function NotFoundPage() {
  return (
    <div className="py-12 text-center">
      <p className="text-sm text-slate-500">This view is planned for Phase 2.3.</p>
      <Link className="mt-4 inline-block text-sm underline underline-offset-4" to="/">
        Back to dashboard
      </Link>
    </div>
  );
}
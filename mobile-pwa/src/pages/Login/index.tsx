import { useState } from 'react';
import { Navigate } from 'react-router-dom';
import { useAuth } from '../../hooks/useAuth';
import { normalizeError } from '../../utils/errorHandler';

export function LoginPage() {
  const { login, isAuthenticated, isPending } = useAuth();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);

  if (isAuthenticated) return <Navigate to="/" replace />;

  const onSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    try {
      await login({ username, password });
    } catch (error) {
      setError(normalizeError(error).title);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-[#0a1e3d] px-4">
      <div className="w-full max-w-sm rounded-lg bg-white p-6 shadow-lg">
        <h1 className="mb-1 text-xl font-bold text-slate-900">ONYX Observer</h1>
        <p className="mb-6 text-sm text-slate-600">
          A read-only view into your organization&apos;s missions. Your account must be granted
          observer access by an administrator.
        </p>
        <form onSubmit={onSubmit} className="space-y-4">
          <label className="block">
            <span className="text-sm font-medium text-slate-700">Username</span>
            <input
              autoComplete="username"
              className="mt-1 block w-full rounded border border-slate-300 px-3 py-2 text-slate-900"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              required
            />
          </label>
          <label className="block">
            <span className="text-sm font-medium text-slate-700">Password</span>
            <input
              type="password"
              autoComplete="current-password"
              className="mt-1 block w-full rounded border border-slate-300 px-3 py-2 text-slate-900"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
            />
          </label>
          {error ? (
            <p className="rounded bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <button
            type="submit"
            className="w-full rounded bg-[#0a1e3d] py-2 text-white hover:bg-[#122a4d] disabled:cursor-not-allowed disabled:opacity-60"
            disabled={isPending}
          >
            {isPending ? 'Signing in…' : 'Sign in'}
          </button>
        </form>
      </div>
    </div>
  );
}
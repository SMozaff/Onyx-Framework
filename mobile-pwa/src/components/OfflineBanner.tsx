import { useOnline } from '../hooks/useOnline';

export function OfflineBanner() {
  const online = useOnline();

  if (online) return null;

  return (
    <div
      role="status"
      data-testid="offline-banner"
      className="border-b border-amber-700 bg-amber-500 px-4 py-2 text-center text-sm font-medium text-white"
    >
      You're offline — showing the last snapshot.
    </div>
  );
}
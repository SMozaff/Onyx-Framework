import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { apiClient } from '../api/client';

export interface PickerUser {
  id: string;
  username: string;
}

interface UserPickerProps {
  label: string;
  value: string;
  onChange: (userId: string) => void;
  placeholder?: string;
  disabled?: boolean;
}

/**
 * Lightweight identity picker shared by the ordinary assignment and staff-loan
 * workflows. It consumes `/api/users`, whose intentionally reduced contract
 * exposes only active same-organization identities — never admin, class, or
 * reporting-line fields.
 */
export default function UserPicker({
  label,
  value,
  onChange,
  placeholder = 'Choose a person',
  disabled = false,
}: UserPickerProps) {
  const [search, setSearch] = useState('');
  const usersQuery = useQuery({
    queryKey: ['users.picker'],
    queryFn: () => apiClient.get<PickerUser[]>('/api/users').then((response) => response.data),
    staleTime: 60_000,
    retry: 1,
    networkMode: 'always',
  });

  const users = useMemo(() => {
    const normalized = search.trim().toLocaleLowerCase();
    if (!normalized) return usersQuery.data ?? [];
    return (usersQuery.data ?? []).filter((user) => user.username.toLocaleLowerCase().includes(normalized));
  }, [search, usersQuery.data]);

  const selectedUnavailable = Boolean(value) && !(usersQuery.data ?? []).some((user) => user.id === value);

  return (
    <label style={{ display: 'grid', gap: 6 }}>
      <span className="muted" style={{ fontSize: 13 }}>
        {label}
      </span>
      <input
        type="search"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        placeholder={`Search ${label.toLocaleLowerCase()}`}
        aria-label={`Search ${label.toLocaleLowerCase()}`}
        disabled={disabled || usersQuery.isLoading}
      />
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        aria-label={label}
        disabled={disabled || usersQuery.isLoading || usersQuery.isError}
      >
        <option value="">{usersQuery.isLoading ? 'Loading people…' : placeholder}</option>
        {selectedUnavailable ? <option value={value}>Previously selected person</option> : null}
        {users.map((user) => (
          <option key={user.id} value={user.id}>
            {user.username}
          </option>
        ))}
      </select>
      {usersQuery.isError ? (
        <span className="muted" role="alert" style={{ fontSize: 13 }}>
          People could not be loaded. Try again shortly.
        </span>
      ) : null}
      {!usersQuery.isLoading && !usersQuery.isError && users.length === 0 ? (
        <span className="muted" style={{ fontSize: 13 }}>
          No people match this search.
        </span>
      ) : null}
    </label>
  );
}

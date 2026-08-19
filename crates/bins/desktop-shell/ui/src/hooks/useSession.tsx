import { createContext, useContext, useMemo } from "react";
import type { ReactNode } from "react";
import type { Id16 } from "@/types/onyx";

/** Exact camelCase shape returned by desktop-shell's `login` and
 * `get_current_session` Tauri commands. It contains non-secret identity
 * data only; access and refresh tokens remain in native secure storage. */
export interface SessionWire {
  username: string;
  isAdmin: boolean;
  serverAddress: string;
  userId: string;
  organizationId: string;
  deviceId: string;
}

/**
 * Session shape consumed by existing desktop pages and command builders.
 * The IDs are converted back to the 16-byte array form the local Rust IPC
 * commands deserialize, while the canonical UUID strings remain available
 * for display and connection settings.
 */
export interface DesktopSession
  extends Omit<SessionWire, "userId" | "organizationId" | "deviceId"> {
  userId: Id16;
  organizationId: Id16;
  deviceId: Id16;
  userIdText: string;
  organizationIdText: string;
  deviceIdText: string;
}

const SessionContext = createContext<DesktopSession | null>(null);

export function SessionProvider({
  session,
  children,
}: {
  session: SessionWire;
  children: ReactNode;
}) {
  const value = useMemo(() => fromWire(session), [session]);
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

/**
 * Returns the authenticated native session. This hook intentionally has no
 * fallback/random-ID path: routing must mount pages only after App has loaded
 * a real persisted session or completed a real login.
 */
// oxlint-disable-next-line react/only-export-components -- This module intentionally exports its paired context provider and consumer hook.
export function useSession(): DesktopSession {
  const session = useContext(SessionContext);
  if (session === null) {
    throw new Error("useSession must be used beneath an authenticated SessionProvider");
  }
  return session;
}

function fromWire(wire: SessionWire): DesktopSession {
  return {
    username: wire.username,
    isAdmin: wire.isAdmin,
    serverAddress: wire.serverAddress,
    userId: uuidToId16(wire.userId),
    organizationId: uuidToId16(wire.organizationId),
    deviceId: uuidToId16(wire.deviceId),
    userIdText: wire.userId,
    organizationIdText: wire.organizationId,
    deviceIdText: wire.deviceId,
  };
}

/** Converts a canonical UUID string into the transparent `[u8; 16]` serde
 * wire shape that ObjectId/OrganizationId use in local Tauri commands. */
function uuidToId16(value: string): Id16 {
  const hexadecimal = value.replace(/-/g, "");
  if (!/^[0-9a-fA-F]{32}$/.test(hexadecimal)) {
    throw new Error(`Native session returned an invalid UUID: ${value}`);
  }

  const bytes: number[] = [];
  for (let index = 0; index < hexadecimal.length; index += 2) {
    bytes.push(Number.parseInt(hexadecimal.slice(index, index + 2), 16));
  }
  return bytes;
}

import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ActorContext,
  AuthorityProof,
  CommandEnvelope,
  DomainObjectRef,
  Id16,
  ShellError,
} from "@/types/onyx";
import { EMPTY_VECTOR_CLOCK, isShellError, newId16 } from "@/types/onyx";

/**
 * Builds a `CommandEnvelope` with every cross-cutting field an actual
 * command execution needs, given only what a UI caller actually knows
 * (the command's own type/payload, the aggregate it targets, and the
 * version/epoch it read the aggregate at). Mirrors the construction
 * pattern from `client-composition`'s own tests
 * (`command_registry.rs`'s `sample_envelope`), adapted for a caller
 * that doesn't yet have a real authenticated session.
 *
 * `actor`/`authorityProof` are placeholders until a real auth/session
 * flow exists (see `desktop-shell/src/lib.rs`'s own
 * `TODO(auth/org resolution)` note — `organization_id` there is
 * currently random-per-launch, matched here by pulling it from the
 * caller rather than fabricating a second, inconsistent one).
 */
function buildEnvelope<TPayload>(params: {
  commandType: string;
  targetId: Id16;
  targetType: string;
  organizationId: Id16;
  userId: Id16;
  deviceId: Id16;
  expectedVersion: number;
  expectedLifecycleEpoch?: number;
  expectedAuthorityEpoch?: number;
  payload: TPayload;
}): CommandEnvelope<TPayload> {
  const target: DomainObjectRef = {
    id: params.targetId,
    type: params.targetType,
    organization_id: params.organizationId,
  };
  const actor: ActorContext = {
    user_id: params.userId,
    device_id: params.deviceId,
    organization_id: params.organizationId,
  };
  const authorityProof: AuthorityProof = {
    proof_type: "Jwt",
    scope: {
      organization_id: params.organizationId,
      object_type: params.targetType,
      object_id: null,
      command_types: [params.commandType],
      delegation_depth: 0,
    },
    issued_at: 0,
    expires_at: Number.MAX_SAFE_INTEGER,
    signature: null,
  };

  return {
    command_id: newId16(),
    operation_id: newId16(),
    command_type: params.commandType,
    schema_version: "1.0.0",
    target,
    expected_version: params.expectedVersion,
    expected_lifecycle_epoch: params.expectedLifecycleEpoch ?? 0,
    expected_authority_epoch: params.expectedAuthorityEpoch ?? 0,
    actor,
    authority_proof: authorityProof,
    issued_at: Date.now() * 1_000_000, // Timestamp is nanoseconds-since-epoch (platform_kernel::time).
    vector_clock: EMPTY_VECTOR_CLOCK,
    correlation_id: newId16(),
    causation_id: null,
    payload: params.payload,
  };
}

export interface UseCommandResult<TPayload> {
  /** Dispatches the command through Tauri's `execute_command`, which
   * wraps `client-composition`'s real `CommandRegistry::dispatch`
   * (`desktop-shell/src/lib.rs`). Resolves with the raw JSON result
   * (`CommandResult`, e.g. `{success, operation_id, new_version, ...}`)
   * on success. */
  execute: (params: {
    commandType: string;
    targetId: Id16;
    targetType: string;
    organizationId: Id16;
    userId: Id16;
    deviceId: Id16;
    expectedVersion: number;
    expectedLifecycleEpoch?: number;
    payload: TPayload;
  }) => Promise<unknown>;
  loading: boolean;
  error: ShellError | null;
}

/**
 * React hook wrapping the `execute_command` Tauri command
 * (`desktop_shell_lib::execute_command`). Mirrors this codebase's own
 * `useQuery` in shape (loading/error state, a single async action
 * function) so the two are consistent to use from a page component.
 */
export function useCommand<TPayload = unknown>(): UseCommandResult<TPayload> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ShellError | null>(null);

  const execute = useCallback<UseCommandResult<TPayload>["execute"]>(async (params) => {
    setLoading(true);
    setError(null);
    try {
      const envelope = buildEnvelope(params);
      // `invoke`'s second argument's keys must match the Tauri command's
      // Rust parameter names exactly (`envelope`, per
      // `execute_command(state, envelope: CommandEnvelope<Value>)` in
      // `desktop-shell/src/lib.rs`) — Tauri's IPC layer maps JS object
      // keys to Rust function parameter names by exact string match.
      return await invoke("execute_command", { envelope });
    } catch (e) {
      const shellError: ShellError = isShellError(e)
        ? e
        : { kind: "command", message: String(e) };
      setError(shellError);
      throw shellError;
    } finally {
      setLoading(false);
    }
  }, []);

  return { execute, loading, error };
}

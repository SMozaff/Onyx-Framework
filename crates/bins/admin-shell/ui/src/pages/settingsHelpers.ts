export type ProjectionState = Record<string, unknown>;

export interface ProjectionConcurrency {
  version: number;
  lifecycleEpoch: number;
  authorityEpoch: number;
}

export type DetailPanelState = "idle" | "loading" | "error" | "empty" | "ready";

export interface SettingsCommand<TPayload> {
  commandType: string;
  targetId: string;
  targetType: "policy" | "legal_hold";
  organizationId: string;
  expectedVersion: number;
  expectedLifecycleEpoch: number;
  expectedAuthorityEpoch: number;
  payload: TPayload;
}

export interface IdempotentCreateOperation<TRequest> {
  key: string;
  payload: TRequest;
}

export interface ApiClientLike {
  post(
    url: string,
    payload: unknown,
    config?: { headers?: Record<string, string> },
  ): Promise<{ data: unknown }>;
}

function asRecord(value: unknown): ProjectionState | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as ProjectionState
    : null;
}

function detailRows(response: unknown): unknown[] {
  if (Array.isArray(response)) return response;
  const record = asRecord(response);
  if (!record) return [];
  if (Array.isArray(record.data)) return record.data;
  if (record.data !== undefined) return [record.data];
  return [record];
}

function candidateState(candidate: unknown): ProjectionState | null {
  const row = asRecord(candidate);
  if (!row) return null;

  const aggregate = asRecord(row.aggregate);
  const nestedState = asRecord(row.state) ?? asRecord(aggregate?.state);
  if (!nestedState) return row;

  // Worker detail responses wrap serialized aggregate state in `state`, while
  // the Rust projection endpoint returns the aggregate state directly. Keep
  // worker row metadata authoritative when present: it contains the current
  // optimistic-concurrency version and both epochs.
  const projection: ProjectionState = { ...nestedState };
  for (const key of ["id", "version", "lifecycle_epoch", "authority_epoch"]) {
    if (row[key] !== undefined) projection[key] = row[key];
  }
  return projection;
}

function projectionId(projection: ProjectionState): string | null {
  return typeof projection.id === "string" && projection.id.length > 0 ? projection.id : null;
}

/** Worker detail queries require `filters.target_id`, not an `id` filter. */
export function detailRequestFilters(targetId: string): { target_id: string } {
  return { target_id: targetId };
}

/**
 * Selects the requested aggregate from either Worker rows (`{ id, state, ...
 * }`) or direct projection state. A mismatched row is never rendered as the
 * requested detail, which protects the action controls from a stale response.
 */
export function detailProjection(response: unknown, requestedId: string): ProjectionState | null {
  const requested = requestedId.toLowerCase();
  for (const row of detailRows(response)) {
    const projection = candidateState(row);
    if (projection && projectionId(projection)?.toLowerCase() === requested) return projection;
  }
  return null;
}

/** The common display state used by both policy and legal-hold detail cards. */
export function detailPanelState(
  requestedId: string | null,
  loading: boolean,
  error: unknown,
  projection: ProjectionState | null,
): DetailPanelState {
  if (!requestedId) return "idle";
  if (loading) return "loading";
  if (error) return "error";
  return projection ? "ready" : "empty";
}

function nonNegativeSafeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

/**
 * Commands are intentionally disabled if any concurrency value is absent or
 * malformed. Falling back to zero would bypass the Worker/Rust epoch checks
 * for aggregates that have already advanced.
 */
export function projectionConcurrency(projection: ProjectionState): ProjectionConcurrency | null {
  const version = nonNegativeSafeInteger(projection.version);
  const lifecycleEpoch = nonNegativeSafeInteger(projection.lifecycle_epoch);
  const authorityEpoch = nonNegativeSafeInteger(projection.authority_epoch);
  return version === null || lifecycleEpoch === null || authorityEpoch === null
    ? null
    : { version, lifecycleEpoch, authorityEpoch };
}

export function policyCommand<TPayload>(
  command: "CreatePolicyVersion" | "PublishPolicyVersion" | "EvaluatePolicy" | "RetirePolicy",
  targetId: string,
  organizationId: string,
  projection: ProjectionState,
  payload: TPayload,
): SettingsCommand<TPayload> | null {
  const concurrency = projectionConcurrency(projection);
  if (!concurrency) return null;
  return {
    commandType: `policy.${command}`,
    targetId,
    targetType: "policy",
    organizationId,
    expectedVersion: concurrency.version,
    expectedLifecycleEpoch: concurrency.lifecycleEpoch,
    expectedAuthorityEpoch: concurrency.authorityEpoch,
    payload,
  };
}

export function legalHoldReleaseCommand(
  targetId: string,
  organizationId: string,
  projection: ProjectionState,
  reason: string,
): SettingsCommand<{ reason: string }> | null {
  const concurrency = projectionConcurrency(projection);
  if (!concurrency) return null;
  return {
    commandType: "legal_hold.ReleaseLegalHold",
    targetId,
    targetType: "legal_hold",
    organizationId,
    expectedVersion: concurrency.version,
    expectedLifecycleEpoch: concurrency.lifecycleEpoch,
    expectedAuthorityEpoch: concurrency.authorityEpoch,
    payload: { reason },
  };
}

export function newCreateOperation<TRequest>(
  payload: TRequest,
  generateKey: () => string = () => crypto.randomUUID(),
): IdempotentCreateOperation<TRequest> {
  return { key: generateKey(), payload };
}

/** Sends the exact Worker REST payload and reuses the operation key on retry. */
export async function postIdempotentCreation<TRequest, TResponse>(
  client: ApiClientLike,
  path: "/api/admin/policies" | "/api/admin/legal-holds",
  operation: IdempotentCreateOperation<TRequest>,
): Promise<TResponse> {
  const response = await client.post(path, operation.payload, {
    headers: { "Idempotency-Key": operation.key },
  });
  return response.data as TResponse;
}

/**
 * A timeout, connection failure, or transient server response may have
 * persisted the request despite the client not receiving its representation.
 * Retrying with the same key lets the Worker replay the canonical result
 * rather than creating another aggregate.
 */
export function isAmbiguousCreateError(error: unknown): boolean {
  const errorRecord = asRecord(error);
  const response = asRecord(errorRecord?.response);
  if (!response) return true;
  const status = response.status;
  if (typeof status === "number" && status >= 500) return true;
  const body = asRecord(response.data);
  return body?.retryability === "TRANSIENT" || body?.retryability === "RETRYABLE";
}

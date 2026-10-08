/**
 * TypeScript mirrors of the real Rust wire types this frontend exchanges
 * with `desktop-shell`'s Tauri commands (`execute_command`,
 * `execute_query`, ...). Every shape here was verified against the
 * actual `platform_kernel`/`platform_contracts` source before being
 * written — including an empirical check of how `serde` actually
 * serializes each type (not assumed from the Rust struct definition
 * alone), since a few of them are non-obvious:
 *
 * - `ObjectId`/`CommandId`/`OperationId`/`CorrelationId`/`EventId` are
 *   all `struct Foo(pub [u8; 16])` newtypes. `serde`'s default
 *   "newtype struct" rule serializes these TRANSPARENTLY as a bare JSON
 *   array of 16 numbers — e.g. `[121,148,25,255,...]`, NOT a UUID
 *   string and NOT `{"0": [...]}`. Confirmed by an isolated
 *   `serde_json::to_string` run against the real type before writing
 *   this file.
 * - `SchemaVersion(pub String)` and `Timestamp(pub u64)` are the same
 *   newtype pattern — they serialize as a bare string / bare number,
 *   not a wrapped object.
 * - `ProofType` is a plain unit-variant enum (`Jwt`, `Paseto`,
 *   `DelegationChain`) with no `#[serde(rename_all = ...)]`, so it
 *   serializes as the bare Rust variant name string, e.g. `"Jwt"`.
 */

/** A 16-byte identifier, exactly as `serde` emits `ObjectId`/`CommandId`/
 * etc: a bare array of 16 numbers. See this file's header comment. */
export type Id16 = number[];

export function newId16(): Id16 {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes);
}

/** Mirrors `platform_kernel::authority::ProofType`. */
export type ProofType = "Jwt" | "Paseto" | "DelegationChain";

/** Mirrors `platform_kernel::authority::AuthorityScope`. */
export interface AuthorityScope {
  organization_id: Id16;
  object_type: string;
  object_id: Id16 | null;
  command_types: string[];
  delegation_depth: number;
}

/** Mirrors `platform_kernel::authority::AuthorityProof`. */
export interface AuthorityProof {
  proof_type: ProofType;
  scope: AuthorityScope;
  issued_at: number;
  expires_at: number;
  signature: number[] | null;
}

/** Mirrors `platform_kernel::refs::DomainObjectRef`. Note the field is
 * literally named `type` on the wire (Rust's `r#type`), which is a
 * reserved word in TypeScript object-literal contexts but fine as a
 * quoted/interface property name. */
export interface DomainObjectRef {
  id: Id16;
  type: string;
  organization_id: Id16;
}

/** Mirrors `platform_kernel::refs::ActorContext` (used identically for
 * both `CommandEnvelope.actor` and inside `AuthorityScope`'s caller —
 * see `mission_domain::test_support::test_context()` for the real
 * construction pattern this mirrors). */
export interface ActorContext {
  user_id: Id16;
  device_id: Id16;
  organization_id: Id16;
}

/** Mirrors `platform_contracts::command::CommandEnvelope<T>`, generic
 * over the JSON payload shape `T` (a `serde_json::Value` on the Rust
 * side, since `execute_command`'s Tauri signature is
 * `CommandEnvelope<serde_json::Value>` — see `desktop-shell/src/lib.rs`). */
export interface CommandEnvelope<TPayload = unknown> {
  command_id: Id16;
  operation_id: Id16;
  command_type: string;
  schema_version: string;
  target: DomainObjectRef;
  expected_version: number;
  expected_lifecycle_epoch: number;
  expected_authority_epoch: number;
  actor: ActorContext;
  authority_proof: AuthorityProof;
  issued_at: number;
  vector_clock: VectorClockJson;
  correlation_id: Id16;
  causation_id: Id16 | null;
  payload: TPayload;
}

/** Mirrors `platform_kernel::causality::VectorClock`'s JSON wire shape
 * per ruling V1 (`DECISIONS.md`): `entries` is a JSON object keyed by
 * each `ReplicaId`'s lowercase 32-character hex string, not the
 * in-memory `BTreeMap<ReplicaId, u64>` shape directly. A freshly-issued
 * command from the desktop shell (which is not itself a sync replica in
 * the causal-ordering sense the way a `SynchronizationSession` peer is)
 * has no meaningful vector clock to attach — sent as `{}` (empty
 * entries), matching `VectorClock::new()`. */
export interface VectorClockJson {
  entries: Record<string, number>;
}

export const EMPTY_VECTOR_CLOCK: VectorClockJson = { entries: {} };

/** Mirrors `client_composition::query_registry::QueryEnvelope`. */
export interface QueryEnvelope {
  query_type: string;
  target_id: Id16;
}

/** Mirrors the `#[serde(tag = "kind", content = "message")]` shape of
 * `desktop_shell_lib::ShellError` (`crates/bins/desktop-shell/src/lib.rs`) —
 * the error type every Tauri command in this app returns on failure,
 * which Tauri's `invoke()` rejects the returned Promise with. */
export interface ShellError {
  kind: "command" | "query" | "storage" | "invalidArgument" | "auth";
  message: string;
}

export function isShellError(e: unknown): e is ShellError {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e &&
    typeof (e as ShellError).message === "string"
  );
}

/** Mirrors `client_composition::sync_agent::SyncStatus`, returned by the
 * `get_sync_status` Tauri command. */
export interface SyncStatus {
  online: boolean;
  pending_outbox_count: number;
  open_conflict_count: number;
  last_sync_attempt: number | null;
  last_sync_result: string | null;
}

/** The subset of `query_application::ports::repository::Loaded` this
 * frontend reads back from `execute_query`'s JSON response
 * (`client_composition::query_registry`'s `LoadedJson` mirror). */
export interface LoadedAggregate {
  aggregate: Record<string, unknown>;
  version: number;
  lifecycle_epoch: number;
  authority_epoch: number;
}

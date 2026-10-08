import type { DomainEventEnvelope } from './events';

export interface DomainObjectRef {
  id: string;
  type: string;
  organization_id: string;
}

export interface ActorContext {
  user_id: string;
  device_id: string;
  organization_id: string;
}

export interface AuthorityScope {
  organization_id: string;
  object_type: string;
  object_id: string | null;
  command_types: string[];
  delegation_depth: number;
}

export interface AuthorityProof {
  proof_type: 'JWT' | 'PASETO' | 'DelegationChain';
  scope: AuthorityScope;
  issued_at: string;
  expires_at: string;
  signature: string | null;
}

export interface VectorClock {
  entries: Record<string, number>;
}

export interface CommandEnvelope<T = unknown> {
  command_id: string;
  operation_id: string;
  command_type: string;
  schema_version: string;
  target: DomainObjectRef;
  expected_version: number;
  expected_lifecycle_epoch: number;
  expected_authority_epoch: number;
  issued_at: string;
  vector_clock: VectorClock;
  correlation_id: string;
  causation_id: string | null;
  payload: T;
  actor?: ActorContext;
  authority_proof?: AuthorityProof;
}

export interface CommandError {
  code: string;
  category: 'DOMAIN' | 'AUTHORITY' | 'CONCURRENCY' | 'INFRASTRUCTURE' | 'VALIDATION';
  retryability: 'RETRYABLE' | 'NON_RETRYABLE' | 'TRANSIENT';
  safe_details: Record<string, string | number | boolean>;
  correlation_id: string;
}

export interface CommandResult<T = unknown> {
  success: boolean;
  operation_id: string;
  new_version?: number;
  new_lifecycle_epoch?: number;
  new_authority_epoch?: number;
  events: DomainEventEnvelope[];
  result?: T;
  error?: CommandError;
}

export interface CommandInput<T = unknown> {
  command_type: 'notification.Acknowledge' | 'approval.Approve' | 'approval.Reject';
  target: DomainObjectRef;
  expected_version: number;
  expected_lifecycle_epoch: number;
  expected_authority_epoch: number;
  payload: T;
}

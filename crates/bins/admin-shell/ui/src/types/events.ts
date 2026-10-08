import type { ActorContext, DomainObjectRef, VectorClock } from './command';

export interface DomainEventEnvelope<T = unknown> {
  event_id: string;
  event_type: string;
  schema_version: string;
  aggregate_ref: DomainObjectRef;
  aggregate_version: number;
  lifecycle_epoch: number;
  authority_epoch: number;
  operation_id: string;
  actor: ActorContext;
  occurred_at: string;
  recorded_at: string;
  vector_clock: VectorClock;
  correlation_id: string;
  causation_id: string | null;
  payload: T;
}

export interface EventSubscriptionFilter {
  event_types?: string[];
  aggregate_ids?: string[];
  organization_id: string;
}

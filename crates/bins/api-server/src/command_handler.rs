//! Command handler pipeline per Team Prompt 2 §6.1.
//! See file-level deviations from the frozen spec in DECISIONS.md (items
//! H1, U1, O1, D1, and the generic-vs-non-generic ruling).

use std::sync::Arc;

use platform_contracts::{
    AggregateRoot, AuditMetadata, DataClassification, DecisionContext, DomainEventEnvelope,
    IdGenerator,
};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectId, ObjectVersion, OperationId, OrganizationId, PolicyDecisionSet, ReplicaId,
    SchemaVersion, Timestamp, UserId, VectorClock, VerifiedAuthority,
};
use query_application::{IdempotencyStore, OutboxMessage, Repository, UnitOfWorkFactory};
use serde::{de::DeserializeOwned, Serialize};

/// A pluggable strategy for answering "may `actor` decide on behalf of
/// `owner`?" — see `handle_command`'s `owner_check` parameter doc
/// comment for the full rationale (why this is a trait rather than a
/// single hardcoded implementation, and why it lives here rather than
/// in a lower-level kernel crate: it needs `async` and needs to be
/// object-safe/`Arc`-shareable across request handlers, neither of
/// which the dependency-light `platform-contracts`/`platform-kernel`
/// crates take on for anything else).
#[async_trait::async_trait]
pub trait OwnerAuthority: Send + Sync {
    async fn is_authorized(&self, actor: UserId, owner: UserId) -> bool;
}

/// `handle_command`'s `owner_check` parameter type, factored out purely
/// to satisfy `clippy::type_complexity` — see that parameter's own doc
/// comment on `handle_command` for what it means and why it's shaped
/// this way.
pub type OwnerCheck<A> = Option<(
    Box<dyn Fn(&A) -> UserId + Send + Sync>,
    Arc<dyn OwnerAuthority>,
)>;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("version conflict: expected {expected:?}, actual {actual:?}")]
    VersionConflict {
        expected: ObjectVersion,
        actual: ObjectVersion,
    },
    #[error("lifecycle epoch conflict: expected {expected:?}, actual {actual:?}")]
    EpochConflict {
        expected: LifecycleEpoch,
        actual: LifecycleEpoch,
    },
    #[error("authority epoch conflict: expected {expected:?}, actual {actual:?}")]
    AuthorityEpochConflict {
        expected: AuthorityEpoch,
        actual: AuthorityEpoch,
    },
    #[error("aggregate not found: {0:?}")]
    NotFound(ObjectId),
    #[error("domain error: {0}")]
    Domain(String),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("idempotency error: {0}")]
    Idempotency(String),
    /// Added alongside `OwnerAuthority` (see that trait's doc comment):
    /// `actor` is not authorized to decide on behalf of `owner`. Kept
    /// distinct from `Domain` so callers (`command_route`'s HTTP
    /// mapping, `ShellError`'s Tauri mapping) can render a specific,
    /// diagnosable message rather than a generic domain-rejection one —
    /// same reasoning as this session's earlier fix for `admin-shell`'s
    /// collapsed error messages.
    #[error("actor {actor:?} is not authorized to decide on behalf of owner {owner:?}")]
    OwnerAuthorityDenied { actor: UserId, owner: UserId },
}

struct DefaultIdGenerator;
impl IdGenerator for DefaultIdGenerator {
    fn generate_object_id(&self) -> ObjectId {
        ObjectId::new_random()
    }
    fn generate_operation_id(&self) -> OperationId {
        OperationId::new_random()
    }
    fn generate_event_id(&self) -> EventId {
        EventId::new_random()
    }
}

pub type CommandResult = serde_json::Value;

/// Execute a command against an aggregate.
/// Generic over aggregate type A (with concrete Command/Event/Error types)
/// but uses non-generic (Value-based) ports per the ruling.
#[allow(clippy::too_many_arguments)]
pub async fn handle_command<A, C, E, Err>(
    command: C,
    target_id: ObjectId,
    operation_id: OperationId,
    actor: ActorContext,
    expected_version: ObjectVersion,
    expected_lifecycle_epoch: LifecycleEpoch,
    expected_authority_epoch: AuthorityEpoch,
    vector_clock: VectorClock,
    correlation_id: CorrelationId,
    aggregate_type_name: &str,
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    idempotency_store: Arc<dyn IdempotencyStore>,
    // `None` for every command that isn't an owner-gated decision (the
    // overwhelming majority of calls) — see the "Step 3b" comment below
    // for the full rationale. `Some((extractor, authority))` pairs a
    // closure that reads the owner id off the already-loaded `A` with
    // the strategy to check that owner against. Boxed rather than a bare
    // generic type parameter so callers that don't need this (i.e.
    // almost all of them) aren't forced to name a concrete no-op type at
    // every call site — `None` alone is enough.
    owner_check: OwnerCheck<A>,
) -> Result<CommandResult, CommandError>
where
    A: AggregateRoot<Command = C, Event = E, Error = Err>,
    A: Serialize + DeserializeOwned,
    E: Serialize,
    Err: std::fmt::Display,
{
    // Step 2: Idempotency check.
    if let Some(cached) = idempotency_store
        .get(&operation_id)
        .await
        .map_err(|e| CommandError::Idempotency(e.to_string()))?
    {
        return Ok(cached);
    }

    let organization_id: OrganizationId = actor.organization_id;

    // Step 3 (ruling H2, DECISIONS.md — reordered from the original Step
    // 3/4 sequence): load the aggregate and validate version/lifecycle
    // epoch BEFORE opening a UnitOfWork transaction. The original
    // ordering opened the transaction first, then called `repo.load()`
    // against the same connection pool — under a pool sized to exactly
    // one connection (the standard, near-mandatory configuration for
    // in-memory SQLite, and a plausible one for a resource-constrained
    // desktop/mobile client), this deadlocks: the open transaction holds
    // the pool's only connection, and `repo.load()` can never acquire one
    // to run its query. Confirmed via isolated reproduction before this
    // fix (see DECISIONS.md ruling H2 for the standalone repro). No
    // transaction is needed for a read, so the load and its
    // read-only version/epoch checks now happen first; the transaction
    // opens only once the caller is actually about to write.
    let loaded = repo
        .load(&target_id)
        .await
        .map_err(|e| CommandError::Persistence(e.to_string()))?;

    let (mut aggregate, current_version) = match loaded {
        Some(l) => {
            let a: A = serde_json::from_value(l.aggregate)?;
            (a, l.version)
        }
        None => return Err(CommandError::NotFound(target_id)),
    };

    if current_version != expected_version {
        return Err(CommandError::VersionConflict {
            expected: expected_version,
            actual: current_version,
        });
    }
    if aggregate.lifecycle_epoch() != expected_lifecycle_epoch {
        return Err(CommandError::EpochConflict {
            expected: expected_lifecycle_epoch,
            actual: aggregate.lifecycle_epoch(),
        });
    }
    if aggregate.authority_epoch() != expected_authority_epoch {
        return Err(CommandError::AuthorityEpochConflict {
            expected: expected_authority_epoch,
            actual: aggregate.authority_epoch(),
        });
    }

    // Step 3b: owner-authority check, for commands that need one.
    //
    // Added to close a real, confirmed gap: `ApproveTask`/`RejectTask`/
    // `ApproveMission`/`RejectMission` previously had no check at all on
    // *who* was issuing them (`decide()`'s own
    // `context.authority.is_authorized(...)` is a documented Increment-1
    // stub that always returns `true` — see `VerifiedAuthority::is_authorized`
    // and the frozen-spec ruling deferring real policy evaluation to
    // Increment 7). Any authenticated org member could approve or reject
    // any task or mission.
    //
    // `owner_check` is `None` for the overwhelming majority of calls
    // (every command that isn't an owner-gated decision) — a no-op in
    // that case. When present, it pairs an extractor closure — over the
    // already-loaded, already-deserialized `aggregate: A` above — with an
    // authority strategy to check the extracted owner against. The
    // extractor is only ever constructed by callers that know `A:
    // HasOwner` concretely (`TaskDecisionHandler`, `MissionDecisionHandler`
    // in `client-composition`, and `command_route`'s HTTP dispatch for the
    // same two commands) — this deliberately avoids adding `A: HasOwner`
    // as a blanket bound on `handle_command` itself, which would force
    // every other aggregate type (`Notification`, `TodoList`,
    // `ApprovalAggregate`, ...) to implement an owner concept that makes
    // no sense for them.
    //
    // The authority strategy itself is caller-supplied, not a single
    // global implementation: `api-server`'s HTTP path has direct database
    // access and can check a live `UserStore`; `desktop-shell`'s
    // offline-first local command path has no local `UserStore` at all
    // and instead checks a locally cached hierarchy snapshot
    // (`desktop_shell::hierarchy::HierarchyCache`). Neither strategy
    // belongs hardcoded into this shared, dependency-light kernel-adjacent
    // crate — each caller supplies its own via the `OwnerAuthority` trait.
    if let Some((get_owner_id, authority)) = owner_check.as_ref() {
        let owner_id = get_owner_id(&aggregate);
        let authorized = authority.is_authorized(actor.user_id, owner_id).await;
        if !authorized {
            return Err(CommandError::OwnerAuthorityDenied {
                actor: actor.user_id,
                owner: owner_id,
            });
        }
    }

    // Step 4 (ruling H2): open the UnitOfWork transaction now that a
    // write is actually about to happen. `unit.rollback()` calls that
    // previously guarded the NotFound/VersionConflict/EpochConflict
    // early-returns above are gone along with them — there is no
    // transaction yet at those points to roll back (they returned before
    // ever reaching this line), which is exactly the point of the
    // reordering: a plain read-and-reject no longer touches the
    // transactional connection pool at all.
    //
    // Race safety: this reordering opens a narrow window between the
    // read-only version check above and the eventual `repo.commit()`
    // below, where a concurrent writer could commit first. This is safe,
    // not merely narrowed: both adapters' `commit()` re-validates
    // optimistic concurrency at write time via a `WHERE version <
    // new_version` guard on the aggregate upsert (verified in both
    // `persistence-{postgres,sqlite}/src/unit_of_work.rs`) — a real
    // concurrent write between load and commit fails there
    // (`sqlx::Error::RowNotFound`, surfaced as `CommandError::Persistence`)
    // rather than silently succeeding with stale data. The early check
    // above was always a fast-path optimization, not the sole guard, in
    // both the original and reordered versions of this function.
    let mut unit = unit_factory
        .create(organization_id)
        .await
        .map_err(|e| CommandError::Persistence(e.to_string()))?;

    // Step 7: Decide (pure, no I/O).
    let ctx = DecisionContext {
        actor: actor.clone(),
        // Team 7 verifies the Ed25519 bearer proof, tenant, object and
        // command scope before entering this generic domain pipeline. The
        // kernel marker remains intentionally data-free for compatibility.
        authority: VerifiedAuthority,
        trusted_now: Timestamp::now(),
        // Rate governance is likewise enforced by Team 7 middleware before
        // this transaction. Domain policy types remain contract-compatible.
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(DefaultIdGenerator),
    };

    let events = aggregate
        .decide(command, &ctx)
        .map_err(|e| CommandError::Domain(e.to_string()))?;

    // Step 8: Apply each produced event to the aggregate (ruling H3,
    // DECISIONS.md — resolves Team 2's own previously-documented "H1"
    // gap, referenced in this file's header comment) and serialize event
    // envelopes in the same pass.
    //
    // `aggregate` is made mutable and `apply()`d in place, rather than
    // cloned as the ruling's own illustrative snippet does — `A` carries
    // no `Clone` bound in this function's `where` clause (only Team 1's
    // concrete `Mission`/`Task` happen to derive it), so `aggregate.clone()`
    // would not compile generically without widening this function's
    // trait bounds, a larger change than this ruling calls for. Mutating
    // in place needs no such bound.
    //
    // Each event is applied via `aggregate.apply(&event)` BEFORE that
    // same event is moved into its envelope's `payload` field — a
    // borrow-then-move within the same loop iteration, needing no `Clone`
    // bound on the event type `E` either (only `E: Serialize`, already
    // required).
    let now = Timestamp::now();
    let aggregate_ref = DomainObjectRef {
        id: target_id,
        r#type: aggregate_type_name.to_string(),
        organization_id,
    };

    let mut event_values: Vec<serde_json::Value> = Vec::with_capacity(events.len());
    for (i, event) in events.into_iter().enumerate() {
        aggregate.apply(&event);

        let event_id = EventId::new_random();
        let event_type = format!("{aggregate_type_name}.event.{i}");
        let envelope = DomainEventEnvelope {
            event_id,
            event_type: event_type.clone(),
            schema_version: SchemaVersion::new("1.0"),
            aggregate_ref: aggregate_ref.clone(),
            aggregate_version: ObjectVersion(current_version.0 + 1 + i as u64),
            lifecycle_epoch: aggregate.lifecycle_epoch(),
            authority_epoch: aggregate.authority_epoch(),
            operation_id,
            actor: actor.clone(),
            occurred_at: now,
            recorded_at: now,
            vector_clock: vector_clock.clone(),
            correlation_id,
            causation_id: None,
            audit_metadata: AuditMetadata {
                source_service: "api-server".to_string(),
                source_replica: ReplicaId::new_random(),
                integrity_digest: None,
                classification: DataClassification::Internal,
                tenant_isolation_key: organization_id,
            },
            payload: event,
        };
        let serialized = serde_json::to_value(&envelope)?;
        unit.register_outbox(OutboxMessage {
            event_id,
            event_type,
            aggregate_id: target_id.to_string(),
            organization_id,
            payload: serde_json::to_vec(&serialized)?,
            vector_clock: vector_clock.clone(),
            occurred_at: now,
        });
        event_values.push(serialized);
    }

    // Step 9 (ruling H3): serialize the aggregate's state AFTER every
    // produced event has been applied, so the persisted `version` field
    // genuinely reflects the new version — closing Team 2's own
    // documented "H1" gap (see this file's header comment), where the
    // pre-apply state was persisted instead, leaving `version` permanently
    // stuck and causing every second write against the same aggregate to
    // fail the `WHERE version < excluded.version` optimistic-concurrency
    // guard in both persistence adapters (confirmed via a real, reproducing
    // integration test before this fix — see DECISIONS.md ruling H3).
    let new_state = serde_json::to_value(&aggregate)?;

    // Step 10: Register and commit.
    repo.commit(new_state, &event_values, &mut *unit)
        .await
        .map_err(|e| CommandError::Persistence(e.to_string()))?;

    let result = serde_json::json!({
        "success": true,
        "operation_id": operation_id,
        "new_version": aggregate.version().0,
        "new_lifecycle_epoch": aggregate.lifecycle_epoch().0,
        "new_authority_epoch": aggregate.authority_epoch().0,
        "event_count": event_values.len(),
        "events": event_values,
    });

    unit.register_idempotency_result(operation_id, result.clone());
    unit.commit()
        .await
        .map_err(|e| CommandError::Persistence(e.to_string()))?;

    Ok(result)
}

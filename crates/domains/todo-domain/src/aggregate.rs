//! The `TodoList`, `TargetList`, and `StaffLoan` aggregate roots.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4, §4.0.1,
//! §4.0.1.1, §4.0.2, §2.1. All product-level mechanics referenced below
//! were resolved through direct, one-at-a-time confirmation with the
//! person, most recently 2026-08-16 — see `DECISIONS.md` for the
//! resolution history in their own words.
//!
//! # Why `TodoList` and `TargetList` are separate aggregate roots
//! Despite sharing a state machine (`state_machine::ListStatus`) and a
//! verification shape (`state_machine::VerificationOutcome`), they are
//! kept as two distinct Rust types rather than one generic
//! `List<Kind>` — a `TodoList` carries discrete `TodoItem`s with no
//! time window; a `TargetList` carries a `TimeWindow` with no items at
//! all, and additionally enforces `WindowNotClosed` before verification
//! (§4.0.2). The overlap is real but the content differs enough that a
//! shared generic would need `Option`-typed fields on both sides to
//! paper over the difference — two thin types with a shared
//! `state_machine` module is more honest about what is actually shared
//! versus merely similar.
//!
//! # Authority (established precedent)
//! All three aggregates follow `mission_domain::Mission`'s precedent
//! exactly: `decide()` checks only the generic
//! `context.authority.is_authorized(...)` stub. Real per-role checks —
//! e.g. "only the parent Manager (or active-loan manager, or escalated
//! authority) may verify this list," "only the real owner may approve a
//! loan request," "only the staff member may approve an extension" —
//! are ABAC/policy-evaluation concerns the composing application
//! enforces before dispatch, per `IMPLEMENTATION_PLAN_User_Hierarchy.md`
//! D.4's verifier-resolution logic and C.2's three-gate approval
//! workflow. This crate does not invent its own authority model.

use crate::command::{StaffLoanCommand, TargetListCommand, TodoListCommand};
use crate::error::{StaffLoanError, TargetError, TodoError};
use crate::event::{StaffLoanEvent, TargetListEvent, TodoListEvent};
use crate::state_machine::{ListStatus, StaffLoanStatus};
use crate::value::{
    ListOrigin, LoanWindow, StaffLoanId, TargetListId, TimeWindow, TodoItem, TodoListId,
};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------
// TodoList
// ---------------------------------------------------------------------

/// The `TodoList` aggregate: a Staff member's list of items, authored
/// either by that Staff member or assigned by a Manager (design doc
/// §4.0.1), verified as a whole — never item-by-item (§4.0.1.1) — by
/// the parent Manager.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoList {
    id: TodoListId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: ListStatus,
    owner: UserId,
    origin: ListOrigin,
    items: Vec<TodoItem>,
    #[allow(dead_code)] // recorded for provenance; no current reader needs it via an accessor
    created_by: UserId,
    /// The recorded Team Leader pre-check, if `RecordTeamLeaderPreCheck`
    /// has been applied. `None` until then. See
    /// `crate::value::TeamLeaderPreCheck`'s doc comment for the
    /// visibility rule this field's *consumers* (not this crate) must
    /// enforce.
    team_leader_pre_check: Option<crate::value::TeamLeaderPreCheck>,
    /// Who this list's escalation (if any) was routed to. `None` unless
    /// `status` is `Escalated`. See
    /// `command::TodoListCommand::EscalateTodoList`'s doc comment for
    /// why this is supplied by the caller rather than computed here.
    escalated_to: Option<UserId>,
}

impl TodoList {
    /// Constructs a new `TodoList` from a `CreateTodoList` command.
    /// Routed through `create()`, not `decide()`, matching every other
    /// domain crate's established creation-command pattern.
    ///
    /// # Errors
    /// Returns [`TodoError::InvalidTransition`] if called with any
    /// command other than `CreateTodoList`.
    pub fn create(
        command: TodoListCommand,
        context: &DecisionContext,
    ) -> Result<Vec<TodoListEvent>, TodoError> {
        let TodoListCommand::CreateTodoList {
            owner,
            origin,
            items,
        } = command
        else {
            return Err(TodoError::InvalidTransition(
                "TodoList::create() called with a non-CreateTodoList command".to_string(),
            ));
        };

        Ok(vec![TodoListEvent::TodoListCreated {
            todo_list_id: TodoListId(context.generated_id_generator.generate_object_id()),
            owner,
            origin,
            items,
            created_by: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `TodoList` by folding its first event
    /// (`TodoListCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `TodoListCreated` event.
    pub fn from_created_event(event: &TodoListEvent) -> Self {
        let TodoListEvent::TodoListCreated {
            todo_list_id,
            owner,
            origin,
            items,
            created_by,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-TodoListCreated event");
        };

        Self {
            id: *todo_list_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: ListStatus::Draft,
            owner: *owner,
            origin: *origin,
            items: items.clone(),
            created_by: *created_by,
            team_leader_pre_check: None,
            escalated_to: None,
        }
    }

    /// The list's current status.
    pub fn status(&self) -> ListStatus {
        self.status
    }

    /// Who this list is for.
    pub fn owner(&self) -> UserId {
        self.owner
    }

    /// Manager-assigned or staff-authored.
    pub fn origin(&self) -> ListOrigin {
        self.origin
    }

    /// The list's current items.
    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    /// The recorded Team Leader pre-check, if any. See
    /// `crate::value::TeamLeaderPreCheck`'s doc comment — this accessor
    /// returns the full record including `notes`; callers responsible
    /// for a Staff-facing view must redact `notes` themselves (design
    /// doc §2.2's visibility rule is a read-model concern, not
    /// something this crate enforces, since it has no concept of "who
    /// is asking").
    pub fn team_leader_pre_check(&self) -> Option<&crate::value::TeamLeaderPreCheck> {
        self.team_leader_pre_check.as_ref()
    }

    /// Who this list's escalation was routed to, if `status` is
    /// `Escalated`. `None` otherwise.
    pub fn escalated_to(&self) -> Option<UserId> {
        self.escalated_to
    }

    fn invalid(&self, command: &str) -> TodoError {
        TodoError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for TodoList {
    type Id = TodoListId;
    type Command = TodoListCommand;
    type Event = TodoListEvent;
    type Error = TodoError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    /// `Verify`/`Reject`/`Escalate` all accept `ListStatus::Escalated`
    /// as a starting status, alongside `Submitted`/`TeamLeaderPreChecked`
    /// -- added 2026-08-16 after live end-to-end testing surfaced a real
    /// bug: `VerifyTodoList` was rejected from `Escalated`, meaning an
    /// escalation target (per design doc section 4.1, resolved
    /// 2026-08-16) could never actually act on what was escalated to
    /// them. Escalate also accepting `Escalated` supports E.1's
    /// confirmed "step-by-step, not a jump" re-escalation -- a stuck
    /// Senior Manager can escalate the same list one hop further, and
    /// each `TodoListEscalated` event simply overwrites `escalated_to`
    /// with the new target (see `apply()` below).
    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("todo.command") {
            return Err(TodoError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            TodoListCommand::CreateTodoList { .. } => Err(TodoError::InvalidTransition(
                "CreateTodoList must be dispatched via TodoList::create(), not decide()"
                    .to_string(),
            )),

            TodoListCommand::AddItem { item } => match self.status {
                ListStatus::Draft => Ok(vec![TodoListEvent::TodoItemAdded {
                    item,
                    added_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("AddItem")),
            },

            TodoListCommand::SubmitTodoList => match self.status {
                ListStatus::Draft => Ok(vec![TodoListEvent::TodoListSubmitted {
                    submitted_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("SubmitTodoList")),
            },

            TodoListCommand::RecordTeamLeaderPreCheck { notes } => match self.status {
                ListStatus::Submitted => Ok(vec![TodoListEvent::TeamLeaderPreCheckRecorded {
                    notes,
                    checked_by: context.actor.user_id,
                    checked_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("RecordTeamLeaderPreCheck")),
            },

            TodoListCommand::VerifyTodoList { outcome, comment } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => Ok(vec![TodoListEvent::TodoListVerified {
                    outcome,
                    comment,
                    verified_by: context.actor.user_id,
                    verified_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("VerifyTodoList")),
            },

            TodoListCommand::RejectTodoList { reason } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => Ok(vec![TodoListEvent::TodoListRejected {
                    reason,
                    rejected_by: context.actor.user_id,
                    rejected_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("RejectTodoList")),
            },

            TodoListCommand::EscalateTodoList {
                reason,
                escalated_to,
            } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => Ok(vec![TodoListEvent::TodoListEscalated {
                    reason,
                    escalated_by: context.actor.user_id,
                    escalated_to,
                    escalated_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("EscalateTodoList")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            TodoListEvent::TodoListCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            TodoListEvent::TodoItemAdded { item, .. } => {
                self.items.push(item.clone());
            }
            TodoListEvent::TodoListSubmitted { .. } => {
                self.status = ListStatus::Submitted;
            }
            TodoListEvent::TeamLeaderPreCheckRecorded {
                notes,
                checked_by,
                checked_at,
            } => {
                self.status = ListStatus::TeamLeaderPreChecked;
                self.team_leader_pre_check = Some(crate::value::TeamLeaderPreCheck {
                    checked_by: *checked_by,
                    checked_at: *checked_at,
                    notes: notes.clone(),
                });
            }
            TodoListEvent::TodoListVerified { .. } => {
                self.status = ListStatus::Verified;
                // Verification is the point at which this list's
                // outcome becomes authoritative/visible — an
                // authority-relevant change, mirroring
                // `policy_domain::Policy`'s `PolicyVersionPublished`
                // handling advancing `authority_epoch` for the same
                // "what's now in effect just changed" reason.
                self.authority_epoch = self.authority_epoch.advance();
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TodoListEvent::TodoListRejected { .. } => {
                self.status = ListStatus::Rejected;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TodoListEvent::TodoListEscalated { escalated_to, .. } => {
                self.status = ListStatus::Escalated;
                self.escalated_to = Some(*escalated_to);
                self.authority_epoch = self.authority_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

// ---------------------------------------------------------------------
// TargetList
// ---------------------------------------------------------------------

/// The `TargetList` aggregate: a time-bound, measurable goal, judged
/// hit or missed by the parent Manager once its window closes (design
/// doc §4.0.2, resolved 2026-08-16). See this file's header comment for
/// why this is a separate type from `TodoList` despite the shared
/// verification shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetList {
    id: TargetListId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: ListStatus,
    owner: UserId,
    origin: ListOrigin,
    description: String,
    time_window: TimeWindow,
    #[allow(dead_code)]
    created_by: UserId,
    /// See `TodoList::team_leader_pre_check`'s doc comment — identical
    /// rationale, identical shape.
    team_leader_pre_check: Option<crate::value::TeamLeaderPreCheck>,
    /// See `TodoList::escalated_to`'s doc comment — identical rationale.
    escalated_to: Option<UserId>,
}

impl TargetList {
    /// Constructs a new `TargetList` from a `CreateTargetList` command.
    ///
    /// # Errors
    /// Returns [`TargetError::InvalidTransition`] if called with any
    /// command other than `CreateTargetList`,
    /// [`TargetError::MissingField`] if `description` is empty, or
    /// [`TargetError::InvalidTimeWindow`] if `time_window.end_at` is
    /// not after `time_window.start_at`.
    pub fn create(
        command: TargetListCommand,
        context: &DecisionContext,
    ) -> Result<Vec<TargetListEvent>, TargetError> {
        let TargetListCommand::CreateTargetList {
            owner,
            origin,
            description,
            time_window,
        } = command
        else {
            return Err(TargetError::InvalidTransition(
                "TargetList::create() called with a non-CreateTargetList command".to_string(),
            ));
        };
        if description.trim().is_empty() {
            return Err(TargetError::MissingField("description".to_string()));
        }
        if time_window.end_at <= time_window.start_at {
            return Err(TargetError::InvalidTimeWindow);
        }

        Ok(vec![TargetListEvent::TargetListCreated {
            target_list_id: TargetListId(context.generated_id_generator.generate_object_id()),
            owner,
            origin,
            description,
            time_window,
            created_by: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `TargetList` by folding its first event
    /// (`TargetListCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `TargetListCreated` event.
    pub fn from_created_event(event: &TargetListEvent) -> Self {
        let TargetListEvent::TargetListCreated {
            target_list_id,
            owner,
            origin,
            description,
            time_window,
            created_by,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-TargetListCreated event");
        };

        Self {
            id: *target_list_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: ListStatus::Draft,
            owner: *owner,
            origin: *origin,
            description: description.clone(),
            time_window: *time_window,
            created_by: *created_by,
            team_leader_pre_check: None,
            escalated_to: None,
        }
    }

    /// The target's current status.
    pub fn status(&self) -> ListStatus {
        self.status
    }

    /// Who this target is for.
    pub fn owner(&self) -> UserId {
        self.owner
    }

    /// The target's fixed time window.
    pub fn time_window(&self) -> TimeWindow {
        self.time_window
    }

    /// See `TodoList::team_leader_pre_check`'s doc comment — identical
    /// rationale.
    pub fn team_leader_pre_check(&self) -> Option<&crate::value::TeamLeaderPreCheck> {
        self.team_leader_pre_check.as_ref()
    }

    /// See `TodoList::escalated_to`'s doc comment — identical rationale.
    pub fn escalated_to(&self) -> Option<UserId> {
        self.escalated_to
    }

    /// What hitting this target means, free text.
    pub fn description(&self) -> &str {
        &self.description
    }

    fn invalid(&self, command: &str) -> TargetError {
        TargetError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for TargetList {
    type Id = TargetListId;
    type Command = TargetListCommand;
    type Event = TargetListEvent;
    type Error = TargetError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    /// See `TodoList::decide`'s doc comment on accepting
    /// `ListStatus::Escalated` -- identical rationale and fix, added
    /// 2026-08-16.
    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("target.command") {
            return Err(TargetError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            TargetListCommand::CreateTargetList { .. } => Err(TargetError::InvalidTransition(
                "CreateTargetList must be dispatched via TargetList::create(), not decide()"
                    .to_string(),
            )),

            TargetListCommand::SubmitTargetList => match self.status {
                ListStatus::Draft => Ok(vec![TargetListEvent::TargetListSubmitted {
                    submitted_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("SubmitTargetList")),
            },

            TargetListCommand::RecordTeamLeaderPreCheck { notes } => match self.status {
                ListStatus::Submitted => Ok(vec![TargetListEvent::TeamLeaderPreCheckRecorded {
                    notes,
                    checked_by: context.actor.user_id,
                    checked_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("RecordTeamLeaderPreCheck")),
            },

            TargetListCommand::VerifyTargetList { outcome, comment } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => {
                    // Design doc §4.0.2, resolved 2026-08-16: hit/miss
                    // is judged only once the window has closed.
                    if context.trusted_now < self.time_window.end_at {
                        return Err(TargetError::WindowNotClosed);
                    }
                    Ok(vec![TargetListEvent::TargetListVerified {
                        outcome,
                        comment,
                        verified_by: context.actor.user_id,
                        verified_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("VerifyTargetList")),
            },

            TargetListCommand::RejectTargetList { reason } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => Ok(vec![TargetListEvent::TargetListRejected {
                    reason,
                    rejected_by: context.actor.user_id,
                    rejected_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("RejectTargetList")),
            },

            TargetListCommand::EscalateTargetList {
                reason,
                escalated_to,
            } => match self.status {
                ListStatus::Submitted
                | ListStatus::TeamLeaderPreChecked
                | ListStatus::Escalated => Ok(vec![TargetListEvent::TargetListEscalated {
                    reason,
                    escalated_by: context.actor.user_id,
                    escalated_to,
                    escalated_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("EscalateTargetList")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            TargetListEvent::TargetListCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            TargetListEvent::TargetListSubmitted { .. } => {
                self.status = ListStatus::Submitted;
            }
            TargetListEvent::TeamLeaderPreCheckRecorded {
                notes,
                checked_by,
                checked_at,
            } => {
                self.status = ListStatus::TeamLeaderPreChecked;
                self.team_leader_pre_check = Some(crate::value::TeamLeaderPreCheck {
                    checked_by: *checked_by,
                    checked_at: *checked_at,
                    notes: notes.clone(),
                });
            }
            TargetListEvent::TargetListVerified { .. } => {
                self.status = ListStatus::Verified;
                self.authority_epoch = self.authority_epoch.advance();
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TargetListEvent::TargetListRejected { .. } => {
                self.status = ListStatus::Rejected;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            TargetListEvent::TargetListEscalated { escalated_to, .. } => {
                self.status = ListStatus::Escalated;
                self.escalated_to = Some(*escalated_to);
                self.authority_epoch = self.authority_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

// ---------------------------------------------------------------------
// StaffLoan
// ---------------------------------------------------------------------

/// The `StaffLoan` aggregate: a time-bounded overlay granting a
/// borrowing Manager working authority over a Staff member owned by
/// another Manager, without transferring ownership (design doc §2.1).
///
/// Three independent approval gates apply across this aggregate's
/// lifecycle — see `command::StaffLoanCommand`'s doc comment for the
/// full breakdown (real owner approves starting; the staff member
/// approves extending; either manager may end early with no approval).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaffLoan {
    id: StaffLoanId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: StaffLoanStatus,
    staff_user_id: UserId,
    real_owner_id: UserId,
    borrowing_manager_id: UserId,
    window: LoanWindow,
    /// Who this loan's approval decision has been escalated to, if
    /// any. `None` unless `EscalateStaffLoan` has been applied. See
    /// `command::StaffLoanCommand::EscalateStaffLoan`'s doc comment.
    escalated_to: Option<UserId>,
}

impl StaffLoan {
    /// Constructs a new `StaffLoan` from a `RequestStaffLoan` command.
    ///
    /// # Errors
    /// Returns [`StaffLoanError::InvalidTransition`] if called with any
    /// command other than `RequestStaffLoan`,
    /// [`StaffLoanError::InvalidLoanWindow`] if `window.end_at` is not
    /// after `window.start_at`, or
    /// [`StaffLoanError::OwnerAndBorrowerMustDiffer`] if
    /// `real_owner_id == borrowing_manager_id`.
    pub fn create(
        command: StaffLoanCommand,
        context: &DecisionContext,
    ) -> Result<Vec<StaffLoanEvent>, StaffLoanError> {
        let StaffLoanCommand::RequestStaffLoan {
            staff_user_id,
            real_owner_id,
            borrowing_manager_id,
            window,
        } = command
        else {
            return Err(StaffLoanError::InvalidTransition(
                "StaffLoan::create() called with a non-RequestStaffLoan command".to_string(),
            ));
        };
        if window.end_at <= window.start_at {
            return Err(StaffLoanError::InvalidLoanWindow);
        }
        if real_owner_id == borrowing_manager_id {
            return Err(StaffLoanError::OwnerAndBorrowerMustDiffer);
        }

        Ok(vec![StaffLoanEvent::StaffLoanRequested {
            staff_loan_id: StaffLoanId(context.generated_id_generator.generate_object_id()),
            staff_user_id,
            real_owner_id,
            borrowing_manager_id,
            window,
            requested_by: context.actor.user_id,
            requested_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `StaffLoan` by folding its first event
    /// (`StaffLoanRequested`).
    ///
    /// # Panics
    /// Panics if `event` is not a `StaffLoanRequested` event.
    pub fn from_created_event(event: &StaffLoanEvent) -> Self {
        let StaffLoanEvent::StaffLoanRequested {
            staff_loan_id,
            staff_user_id,
            real_owner_id,
            borrowing_manager_id,
            window,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-StaffLoanRequested event");
        };

        Self {
            id: *staff_loan_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: StaffLoanStatus::Requested,
            staff_user_id: *staff_user_id,
            real_owner_id: *real_owner_id,
            borrowing_manager_id: *borrowing_manager_id,
            window: *window,
            escalated_to: None,
        }
    }

    /// The loan's current status.
    pub fn status(&self) -> StaffLoanStatus {
        self.status
    }

    /// The staff member being loaned.
    pub fn staff_user_id(&self) -> UserId {
        self.staff_user_id
    }

    /// The staff member's real owning manager.
    pub fn real_owner_id(&self) -> UserId {
        self.real_owner_id
    }

    /// The manager borrowing the staff member.
    pub fn borrowing_manager_id(&self) -> UserId {
        self.borrowing_manager_id
    }

    /// The loan's current window (reflects extensions, if any).
    pub fn window(&self) -> LoanWindow {
        self.window
    }

    /// Whether `manager_id` currently holds verification authority over
    /// this loan's staff member — i.e. is either the real owner or the
    /// borrowing manager, and the loan is `Active` or `Extended`.
    /// Design doc §2.1's confirmed "either can verify" rule; exposed as
    /// a direct helper since `IMPLEMENTATION_PLAN_User_Hierarchy.md`
    /// D.4 calls this a natural chokepoint other phases should share
    /// rather than re-derive.
    pub fn grants_verification_authority_to(&self, manager_id: UserId) -> bool {
        matches!(
            self.status,
            StaffLoanStatus::Active | StaffLoanStatus::Extended
        ) && (manager_id == self.real_owner_id || manager_id == self.borrowing_manager_id)
    }

    /// Who this loan's approval decision has been escalated to, if any.
    pub fn escalated_to(&self) -> Option<UserId> {
        self.escalated_to
    }

    /// Whether `manager_id` may currently `ApproveStaffLoan`/
    /// `DeclineStaffLoan` this `Requested` loan: the real owner always
    /// may (design doc §2.1), and if this loan has been escalated, the
    /// escalation target may **instead** — confirmed by the person
    /// 2026-08-16: escalating a stuck loan request gives the escalation
    /// target real authority to decide in the real owner's place, not
    /// merely a notification. Mirrors
    /// `verifier_resolution`'s "escalation target replaces the normal
    /// authority for this specific item" rule from the Todo/Target side
    /// of Phase E, applied here to the loan's own approval decision.
    pub fn grants_approval_authority_to(&self, manager_id: UserId) -> bool {
        if self.status != StaffLoanStatus::Requested {
            return false;
        }
        match self.escalated_to {
            Some(target) => manager_id == target,
            None => manager_id == self.real_owner_id,
        }
    }

    fn invalid(&self, command: &str) -> StaffLoanError {
        StaffLoanError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for StaffLoan {
    type Id = StaffLoanId;
    type Command = StaffLoanCommand;
    type Event = StaffLoanEvent;
    type Error = StaffLoanError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("staff_loan.command") {
            return Err(StaffLoanError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            StaffLoanCommand::RequestStaffLoan { .. } => Err(StaffLoanError::InvalidTransition(
                "RequestStaffLoan must be dispatched via StaffLoan::create(), not decide()"
                    .to_string(),
            )),

            // Design doc §2.1, resolved 2026-08-16: only the real
            // owner may approve/decline — enforced by the composing
            // application (see this file's header comment), not here.
            StaffLoanCommand::ApproveStaffLoan => match self.status {
                StaffLoanStatus::Requested => Ok(vec![StaffLoanEvent::StaffLoanApproved {
                    approved_by: context.actor.user_id,
                    approved_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("ApproveStaffLoan")),
            },

            StaffLoanCommand::DeclineStaffLoan { reason } => match self.status {
                StaffLoanStatus::Requested => Ok(vec![StaffLoanEvent::StaffLoanDeclined {
                    reason,
                    declined_by: context.actor.user_id,
                    declined_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("DeclineStaffLoan")),
            },

            // Design doc §2.1, resolved 2026-08-16: represents the
            // staff member's own approval, not either manager's —
            // enforced by the composing application.
            StaffLoanCommand::ExtendStaffLoan { new_end_at } => match self.status {
                StaffLoanStatus::Active | StaffLoanStatus::Extended => {
                    if new_end_at <= self.window.end_at {
                        return Err(StaffLoanError::InvalidLoanWindow);
                    }
                    Ok(vec![StaffLoanEvent::StaffLoanExtended {
                        new_end_at,
                        approved_by_staff: context.actor.user_id,
                        extended_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("ExtendStaffLoan")),
            },

            // Design doc §2.1, resolved 2026-08-16: no approval
            // required from anyone; either manager may invoke this.
            StaffLoanCommand::EndStaffLoanEarly => match self.status {
                StaffLoanStatus::Active | StaffLoanStatus::Extended => {
                    Ok(vec![StaffLoanEvent::StaffLoanEndedEarly {
                        ended_by: context.actor.user_id,
                        ended_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("EndStaffLoanEarly")),
            },

            // Invoked only by the scheduled background job (design doc
            // §2.1, resolved 2026-08-16) — see this crate's docs.
            StaffLoanCommand::ExpireStaffLoan => match self.status {
                StaffLoanStatus::Active | StaffLoanStatus::Extended => {
                    Ok(vec![StaffLoanEvent::StaffLoanExpired {
                        expired_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("ExpireStaffLoan")),
            },

            // Design doc E.2, resolved 2026-08-16: staff-loan approval
            // is escalatable. Only valid from `Requested` -- every
            // other status is already a resolved decision (Active,
            // Declined, etc.) with nothing left to escalate. Supports
            // re-escalation the same way TodoList/TargetList do: a
            // second `EscalateStaffLoan` while already escalated simply
            // overwrites `escalated_to` (see `apply()` below).
            StaffLoanCommand::EscalateStaffLoan {
                reason,
                escalated_to,
            } => match self.status {
                StaffLoanStatus::Requested => Ok(vec![StaffLoanEvent::StaffLoanEscalated {
                    reason,
                    escalated_by: context.actor.user_id,
                    escalated_to,
                    escalated_at: context.trusted_now,
                }]),
                _ => Err(self.invalid("EscalateStaffLoan")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            StaffLoanEvent::StaffLoanRequested { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            StaffLoanEvent::StaffLoanApproved { .. } => {
                self.status = StaffLoanStatus::Active;
                // Approval is what actually grants the borrowing
                // manager verification authority — an
                // authority-relevant change.
                self.authority_epoch = self.authority_epoch.advance();
            }
            StaffLoanEvent::StaffLoanDeclined { .. } => {
                self.status = StaffLoanStatus::Declined;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            StaffLoanEvent::StaffLoanExtended { new_end_at, .. } => {
                self.status = StaffLoanStatus::Extended;
                self.window.end_at = *new_end_at;
            }
            StaffLoanEvent::StaffLoanEndedEarly { .. } => {
                self.status = StaffLoanStatus::Ended;
                // Ends the borrowing manager's verification authority —
                // authority-relevant.
                self.authority_epoch = self.authority_epoch.advance();
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            StaffLoanEvent::StaffLoanExpired { .. } => {
                self.status = StaffLoanStatus::Expired;
                self.authority_epoch = self.authority_epoch.advance();
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            StaffLoanEvent::StaffLoanEscalated { escalated_to, .. } => {
                // Status stays Requested -- escalation widens/replaces
                // who may approve/decline, it does not itself resolve
                // the decision. See `grants_approval_authority_to`'s
                // doc comment.
                self.escalated_to = Some(*escalated_to);
                self.authority_epoch = self.authority_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::VerificationOutcome;
    use crate::test_support::{
        active_staff_loan, draft_target_list, draft_todo_list, open_time_window,
        requested_staff_loan, submitted_target_list, submitted_todo_list, test_context,
        test_context_at, test_user_id, todo_item, two_week_loan_window,
    };
    use platform_kernel::Timestamp;

    fn apply_all<A: AggregateRoot>(agg: &mut A, events: &[A::Event]) {
        for e in events {
            agg.apply(e);
        }
    }

    // -------------------------------------------------------------
    // TodoList
    // -------------------------------------------------------------

    #[test]
    fn create_todo_list_succeeds() {
        let list = draft_todo_list();
        assert_eq!(list.status(), ListStatus::Draft);
        assert_eq!(list.items().len(), 1);
        assert_eq!(list.origin(), ListOrigin::StaffAuthored);
    }

    #[test]
    fn create_todo_list_via_decide_is_rejected() {
        let list = draft_todo_list();
        let ctx = test_context();
        let result = list.decide(
            TodoListCommand::CreateTodoList {
                owner: test_user_id(),
                origin: ListOrigin::StaffAuthored,
                items: vec![],
            },
            &ctx,
        );
        assert!(matches!(result, Err(TodoError::InvalidTransition(_))));
    }

    #[test]
    fn add_item_on_draft_succeeds() {
        let mut list = draft_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::AddItem {
                    item: todo_item("Second item"),
                },
                &ctx,
            )
            .expect("add item must succeed on Draft");
        apply_all(&mut list, &events);
        assert_eq!(list.items().len(), 2);
    }

    #[test]
    fn add_item_after_submission_is_rejected() {
        let list = submitted_todo_list();
        let ctx = test_context();
        let result = list.decide(
            TodoListCommand::AddItem {
                item: todo_item("Too late"),
            },
            &ctx,
        );
        assert!(matches!(result, Err(TodoError::InvalidTransition(_))));
        assert_eq!(list.status(), ListStatus::Submitted);
    }

    #[test]
    fn submit_draft_todo_list_succeeds() {
        let mut list = draft_todo_list();
        let ctx = test_context();
        let events = list
            .decide(TodoListCommand::SubmitTodoList, &ctx)
            .expect("submit must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Submitted);
    }

    #[test]
    fn submit_already_submitted_todo_list_is_rejected() {
        let list = submitted_todo_list();
        let ctx = test_context();
        let result = list.decide(TodoListCommand::SubmitTodoList, &ctx);
        assert!(matches!(result, Err(TodoError::InvalidTransition(_))));
    }

    #[test]
    fn team_leader_pre_check_is_non_gating() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed without a pre-check");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn todo_list_has_no_pre_check_before_one_is_recorded() {
        let list = submitted_todo_list();
        assert!(list.team_leader_pre_check().is_none());
    }

    #[test]
    fn team_leader_pre_check_then_verify_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let precheck_events = list
            .decide(
                TodoListCommand::RecordTeamLeaderPreCheck {
                    notes: "Looks fine, minor formatting issue".to_string(),
                },
                &ctx,
            )
            .expect("pre-check must succeed");
        apply_all(&mut list, &precheck_events);
        assert_eq!(list.status(), ListStatus::TeamLeaderPreChecked);
        // Regression check: `apply()` must actually store the
        // pre-check's data (checked_by/checked_at/notes), not merely
        // transition status. This was a real bug found 2026-08-16 while
        // building D.5 redaction — the event's fields were previously
        // discarded via a `{ .. }` pattern, leaving nothing for
        // redaction logic to redact.
        let recorded = list
            .team_leader_pre_check()
            .expect("pre-check data must be stored on the aggregate");
        assert_eq!(recorded.notes, "Looks fine, minor formatting issue");
        assert_eq!(recorded.checked_by, ctx.actor.user_id);

        let verify_events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed after a pre-check");
        apply_all(&mut list, &verify_events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn verify_flawless_with_no_comment_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn verify_flawless_with_a_comment_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::Flawless,
                    comment: Some("Great work, small note for next time".to_string()),
                },
                &ctx,
            )
            .expect("verify must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn verify_with_deficiencies_and_no_comment_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::WithDeficiencies,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed even without a comment");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn reject_submitted_todo_list_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let events = list
            .decide(
                TodoListCommand::RejectTodoList {
                    reason: "Wrong list entirely".to_string(),
                },
                &ctx,
            )
            .expect("reject must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Rejected);
    }

    #[test]
    fn verify_draft_todo_list_is_rejected() {
        let list = draft_todo_list();
        let ctx = test_context();
        let result = list.decide(
            TodoListCommand::VerifyTodoList {
                outcome: VerificationOutcome::Flawless,
                comment: None,
            },
            &ctx,
        );
        assert!(matches!(result, Err(TodoError::InvalidTransition(_))));
    }

    #[test]
    fn escalate_submitted_todo_list_succeeds() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let events = list
            .decide(
                TodoListCommand::EscalateTodoList {
                    reason: "Parent manager unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Escalated);
        assert_eq!(list.escalated_to(), Some(escalation_target));
    }

    #[test]
    fn manager_assigned_todo_list_still_requires_verification() {
        let ctx = test_context();
        let owner = test_user_id();
        let events = TodoList::create(
            TodoListCommand::CreateTodoList {
                owner,
                origin: ListOrigin::ManagerAssigned,
                items: vec![todo_item("Assigned task")],
            },
            &ctx,
        )
        .expect("create must succeed");
        let mut list = TodoList::from_created_event(&events[0]);
        assert_eq!(list.origin(), ListOrigin::ManagerAssigned);

        let submit_events = list
            .decide(TodoListCommand::SubmitTodoList, &ctx)
            .expect("submit must succeed");
        apply_all(&mut list, &submit_events);
        assert_eq!(
            list.status(),
            ListStatus::Submitted,
            "a Manager-assigned list must still go through the same Submitted state"
        );
    }

    // -------------------------------------------------------------
    // TargetList
    // -------------------------------------------------------------

    #[test]
    fn create_target_list_succeeds() {
        let list = draft_target_list();
        assert_eq!(list.status(), ListStatus::Draft);
        assert_eq!(list.time_window(), open_time_window());
    }

    #[test]
    fn target_list_team_leader_pre_check_is_stored() {
        // Same regression coverage as
        // `team_leader_pre_check_then_verify_succeeds` above, for
        // TargetList's independent apply() implementation.
        let mut list = submitted_target_list();
        let ctx = test_context();
        let events = list
            .decide(
                TargetListCommand::RecordTeamLeaderPreCheck {
                    notes: "Target looks achievable".to_string(),
                },
                &ctx,
            )
            .expect("pre-check must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::TeamLeaderPreChecked);
        let recorded = list
            .team_leader_pre_check()
            .expect("pre-check data must be stored on the aggregate");
        assert_eq!(recorded.notes, "Target looks achievable");
        assert_eq!(recorded.checked_by, ctx.actor.user_id);
    }

    #[test]
    fn escalated_todo_list_can_be_verified() {
        // Regression test for a real bug found via live end-to-end
        // testing 2026-08-16: VerifyTodoList was rejected from
        // Escalated, meaning the escalation target could never act on
        // what was escalated to them.
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let escalate_events = list
            .decide(
                TodoListCommand::EscalateTodoList {
                    reason: "Parent manager unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut list, &escalate_events);
        assert_eq!(list.status(), ListStatus::Escalated);

        let verify_events = list
            .decide(
                TodoListCommand::VerifyTodoList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed from Escalated -- this is the whole point of escalation");
        apply_all(&mut list, &verify_events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn escalated_todo_list_can_be_rejected() {
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let escalate_events = list
            .decide(
                TodoListCommand::EscalateTodoList {
                    reason: "Parent manager unreachable".to_string(),
                    escalated_to: test_user_id(),
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut list, &escalate_events);

        let reject_events = list
            .decide(
                TodoListCommand::RejectTodoList {
                    reason: "Not acceptable even at this level".to_string(),
                },
                &ctx,
            )
            .expect("reject must succeed from Escalated");
        apply_all(&mut list, &reject_events);
        assert_eq!(list.status(), ListStatus::Rejected);
    }

    #[test]
    fn escalated_todo_list_can_be_re_escalated() {
        // E.1's confirmed "step-by-step, not a jump" re-escalation --
        // a stuck escalation target can escalate the same list one hop
        // further, and the new escalated_to overwrites the old one.
        let mut list = submitted_todo_list();
        let ctx = test_context();
        let first_target = test_user_id();
        let first_events = list
            .decide(
                TodoListCommand::EscalateTodoList {
                    reason: "First escalation".to_string(),
                    escalated_to: first_target,
                },
                &ctx,
            )
            .expect("first escalate must succeed");
        apply_all(&mut list, &first_events);
        assert_eq!(list.escalated_to(), Some(first_target));

        let second_target = test_user_id();
        assert_ne!(first_target, second_target);
        let second_events = list
            .decide(
                TodoListCommand::EscalateTodoList {
                    reason: "Still stuck, escalating further".to_string(),
                    escalated_to: second_target,
                },
                &ctx,
            )
            .expect("re-escalate must succeed from Escalated");
        apply_all(&mut list, &second_events);
        assert_eq!(list.status(), ListStatus::Escalated);
        assert_eq!(
            list.escalated_to(),
            Some(second_target),
            "re-escalation must overwrite escalated_to with the new target, not keep the old one"
        );
    }

    #[test]
    fn escalate_submitted_target_list_succeeds() {
        let mut list = submitted_target_list();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let events = list
            .decide(
                TargetListCommand::EscalateTargetList {
                    reason: "Parent manager unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Escalated);
        assert_eq!(list.escalated_to(), Some(escalation_target));
    }

    #[test]
    fn escalated_target_list_can_be_verified() {
        // Same regression coverage as escalated_todo_list_can_be_verified,
        // for TargetList's independent decide() guard. Must verify
        // after the window closes (WindowNotClosed still applies
        // regardless of escalation -- escalation changes who may
        // verify, not when).
        let mut list = submitted_target_list();
        let window = list.time_window();
        let ctx = test_context_at(window.end_at);
        let escalation_target = test_user_id();
        let escalate_events = list
            .decide(
                TargetListCommand::EscalateTargetList {
                    reason: "Parent manager unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut list, &escalate_events);
        assert_eq!(list.status(), ListStatus::Escalated);

        let verify_events = list
            .decide(
                TargetListCommand::VerifyTargetList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed from Escalated after the window closes");
        apply_all(&mut list, &verify_events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn create_target_list_with_empty_description_is_rejected() {
        let ctx = test_context();
        let result = TargetList::create(
            TargetListCommand::CreateTargetList {
                owner: test_user_id(),
                origin: ListOrigin::StaffAuthored,
                description: "   ".to_string(),
                time_window: open_time_window(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(TargetError::MissingField(_))));
    }

    #[test]
    fn create_target_list_with_inverted_window_is_rejected() {
        let ctx = test_context();
        let inverted = TimeWindow {
            start_at: open_time_window().end_at,
            end_at: open_time_window().start_at,
        };
        let result = TargetList::create(
            TargetListCommand::CreateTargetList {
                owner: test_user_id(),
                origin: ListOrigin::StaffAuthored,
                description: "Bad window".to_string(),
                time_window: inverted,
            },
            &ctx,
        );
        assert!(matches!(result, Err(TargetError::InvalidTimeWindow)));
    }

    #[test]
    fn verify_target_before_window_closes_is_rejected() {
        let list = submitted_target_list();
        let window = list.time_window();
        let ctx = test_context_at(Timestamp(window.end_at.0 - 1));
        let result = list.decide(
            TargetListCommand::VerifyTargetList {
                outcome: VerificationOutcome::Flawless,
                comment: None,
            },
            &ctx,
        );
        assert!(matches!(result, Err(TargetError::WindowNotClosed)));
        assert_eq!(list.status(), ListStatus::Submitted);
    }

    #[test]
    fn verify_target_at_exact_window_close_succeeds() {
        let mut list = submitted_target_list();
        let window = list.time_window();
        let ctx = test_context_at(window.end_at);
        let events = list
            .decide(
                TargetListCommand::VerifyTargetList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed exactly at window close");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn verify_target_hit_after_window_closes_succeeds() {
        let mut list = submitted_target_list();
        let window = list.time_window();
        let ctx = test_context_at(Timestamp(window.end_at.0 + 1));
        let events = list
            .decide(
                TargetListCommand::VerifyTargetList {
                    outcome: VerificationOutcome::Flawless,
                    comment: None,
                },
                &ctx,
            )
            .expect("verify must succeed after window close");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn verify_target_miss_after_window_closes_succeeds() {
        let mut list = submitted_target_list();
        let window = list.time_window();
        let ctx = test_context_at(Timestamp(window.end_at.0 + 1));
        let events = list
            .decide(
                TargetListCommand::VerifyTargetList {
                    outcome: VerificationOutcome::WithDeficiencies,
                    comment: Some("Fell short by 2 tickets".to_string()),
                },
                &ctx,
            )
            .expect("verify must succeed after window close");
        apply_all(&mut list, &events);
        assert_eq!(list.status(), ListStatus::Verified);
    }

    #[test]
    fn target_list_has_no_per_item_state() {
        let list = draft_target_list();
        let _ = list.status();
    }

    // -------------------------------------------------------------
    // StaffLoan
    // -------------------------------------------------------------

    #[test]
    fn request_staff_loan_succeeds() {
        let loan = requested_staff_loan();
        assert_eq!(loan.status(), StaffLoanStatus::Requested);
        assert_eq!(loan.window(), two_week_loan_window());
    }

    #[test]
    fn request_staff_loan_with_inverted_window_is_rejected() {
        let ctx = test_context();
        let inverted = LoanWindow {
            start_at: two_week_loan_window().end_at,
            end_at: two_week_loan_window().start_at,
        };
        let result = StaffLoan::create(
            StaffLoanCommand::RequestStaffLoan {
                staff_user_id: test_user_id(),
                real_owner_id: test_user_id(),
                borrowing_manager_id: test_user_id(),
                window: inverted,
            },
            &ctx,
        );
        assert!(matches!(result, Err(StaffLoanError::InvalidLoanWindow)));
    }

    #[test]
    fn request_staff_loan_with_same_owner_and_borrower_is_rejected() {
        let ctx = test_context();
        let same_manager = test_user_id();
        let result = StaffLoan::create(
            StaffLoanCommand::RequestStaffLoan {
                staff_user_id: test_user_id(),
                real_owner_id: same_manager,
                borrowing_manager_id: same_manager,
                window: two_week_loan_window(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(StaffLoanError::OwnerAndBorrowerMustDiffer)
        ));
    }

    #[test]
    fn owner_approves_requested_loan_succeeds() {
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let events = loan
            .decide(StaffLoanCommand::ApproveStaffLoan, &ctx)
            .expect("approve must succeed");
        apply_all(&mut loan, &events);
        assert_eq!(loan.status(), StaffLoanStatus::Active);
    }

    #[test]
    fn owner_declines_requested_loan_succeeds() {
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let events = loan
            .decide(
                StaffLoanCommand::DeclineStaffLoan {
                    reason: "Need them this cycle".to_string(),
                },
                &ctx,
            )
            .expect("decline must succeed");
        apply_all(&mut loan, &events);
        assert_eq!(loan.status(), StaffLoanStatus::Declined);
    }

    #[test]
    fn approve_already_active_loan_is_rejected() {
        let loan = active_staff_loan();
        let ctx = test_context();
        let result = loan.decide(StaffLoanCommand::ApproveStaffLoan, &ctx);
        assert!(matches!(result, Err(StaffLoanError::InvalidTransition(_))));
    }

    #[test]
    fn active_loan_grants_verification_authority_to_both_managers() {
        let loan = active_staff_loan();
        assert!(loan.grants_verification_authority_to(loan.real_owner_id()));
        assert!(loan.grants_verification_authority_to(loan.borrowing_manager_id()));
        assert!(!loan.grants_verification_authority_to(test_user_id()));
    }

    #[test]
    fn requested_loan_grants_no_verification_authority_yet() {
        let loan = requested_staff_loan();
        assert!(!loan.grants_verification_authority_to(loan.real_owner_id()));
        assert!(!loan.grants_verification_authority_to(loan.borrowing_manager_id()));
    }

    #[test]
    fn extend_active_loan_succeeds() {
        let mut loan = active_staff_loan();
        let ctx = test_context();
        let original_end = loan.window().end_at;
        let new_end = Timestamp(original_end.0 + 86_400_000_000_000);
        let events = loan
            .decide(
                StaffLoanCommand::ExtendStaffLoan {
                    new_end_at: new_end,
                },
                &ctx,
            )
            .expect("extend must succeed");
        apply_all(&mut loan, &events);
        assert_eq!(loan.status(), StaffLoanStatus::Extended);
        assert_eq!(loan.window().end_at, new_end);
    }

    #[test]
    fn extend_with_earlier_end_is_rejected() {
        let loan = active_staff_loan();
        let ctx = test_context();
        let earlier = Timestamp(loan.window().start_at.0 + 1);
        let result = loan.decide(
            StaffLoanCommand::ExtendStaffLoan {
                new_end_at: earlier,
            },
            &ctx,
        );
        assert!(matches!(result, Err(StaffLoanError::InvalidLoanWindow)));
    }

    #[test]
    fn extend_a_requested_but_not_yet_active_loan_is_rejected() {
        let loan = requested_staff_loan();
        let ctx = test_context();
        let new_end = Timestamp(loan.window().end_at.0 + 86_400_000_000_000);
        let result = loan.decide(
            StaffLoanCommand::ExtendStaffLoan {
                new_end_at: new_end,
            },
            &ctx,
        );
        assert!(matches!(result, Err(StaffLoanError::InvalidTransition(_))));
    }

    #[test]
    fn end_active_loan_early_requires_no_approval_state() {
        let mut loan = active_staff_loan();
        let ctx = test_context();
        let events = loan
            .decide(StaffLoanCommand::EndStaffLoanEarly, &ctx)
            .expect("end early must succeed unilaterally");
        apply_all(&mut loan, &events);
        assert_eq!(loan.status(), StaffLoanStatus::Ended);
    }

    #[test]
    fn ended_loan_grants_no_further_verification_authority() {
        let mut loan = active_staff_loan();
        let ctx = test_context();
        let events = loan
            .decide(StaffLoanCommand::EndStaffLoanEarly, &ctx)
            .expect("end early must succeed");
        apply_all(&mut loan, &events);
        assert!(!loan.grants_verification_authority_to(loan.borrowing_manager_id()));
    }

    #[test]
    fn end_extended_loan_early_also_succeeds() {
        let mut loan = active_staff_loan();
        let ctx = test_context();
        let extend_events = loan
            .decide(
                StaffLoanCommand::ExtendStaffLoan {
                    new_end_at: Timestamp(loan.window().end_at.0 + 86_400_000_000_000),
                },
                &ctx,
            )
            .expect("extend must succeed");
        apply_all(&mut loan, &extend_events);

        let end_events = loan
            .decide(StaffLoanCommand::EndStaffLoanEarly, &ctx)
            .expect("end early must succeed even after an extension");
        apply_all(&mut loan, &end_events);
        assert_eq!(loan.status(), StaffLoanStatus::Ended);
    }

    #[test]
    fn expire_active_loan_succeeds() {
        let mut loan = active_staff_loan();
        let ctx = test_context_at(loan.window().end_at);
        let events = loan
            .decide(StaffLoanCommand::ExpireStaffLoan, &ctx)
            .expect("expire must succeed");
        apply_all(&mut loan, &events);
        assert_eq!(loan.status(), StaffLoanStatus::Expired);
    }

    #[test]
    fn expire_requested_but_not_active_loan_is_rejected() {
        let loan = requested_staff_loan();
        let ctx = test_context();
        let result = loan.decide(StaffLoanCommand::ExpireStaffLoan, &ctx);
        assert!(matches!(result, Err(StaffLoanError::InvalidTransition(_))));
    }

    #[test]
    fn expired_loan_grants_no_further_verification_authority() {
        let mut loan = active_staff_loan();
        let ctx = test_context_at(loan.window().end_at);
        let events = loan
            .decide(StaffLoanCommand::ExpireStaffLoan, &ctx)
            .expect("expire must succeed");
        apply_all(&mut loan, &events);
        assert!(!loan.grants_verification_authority_to(loan.real_owner_id()));
    }

    #[test]
    fn escalate_requested_staff_loan_succeeds() {
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let events = loan
            .decide(
                StaffLoanCommand::EscalateStaffLoan {
                    reason: "Real owner unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut loan, &events);
        assert_eq!(
            loan.status(),
            StaffLoanStatus::Requested,
            "escalation widens who may decide, it does not itself resolve the decision"
        );
        assert_eq!(loan.escalated_to(), Some(escalation_target));
    }

    #[test]
    fn escalate_active_staff_loan_is_rejected() {
        // Only a stuck (Requested) approval decision can be escalated
        // -- an already-decided loan has nothing left to escalate.
        let loan = active_staff_loan();
        let ctx = test_context();
        let result = loan.decide(
            StaffLoanCommand::EscalateStaffLoan {
                reason: "irrelevant".to_string(),
                escalated_to: test_user_id(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(StaffLoanError::InvalidTransition(_))));
    }

    #[test]
    fn escalation_target_gains_approval_authority_in_place_of_real_owner() {
        // Confirmed by the person 2026-08-16: the escalation target
        // gains real authority to approve/decline, not merely a
        // notification.
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let events = loan
            .decide(
                StaffLoanCommand::EscalateStaffLoan {
                    reason: "Real owner unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut loan, &events);

        assert!(
            loan.grants_approval_authority_to(escalation_target),
            "the escalation target must gain approval authority"
        );
        assert!(
            !loan.grants_approval_authority_to(loan.real_owner_id()),
            "the real owner must lose approval authority once escalated -- \
             the escalation target decides in their place, per the person's \
             explicit confirmation, not alongside them"
        );
    }

    #[test]
    fn unescalated_requested_loan_grants_approval_authority_only_to_real_owner() {
        let loan = requested_staff_loan();
        assert!(loan.grants_approval_authority_to(loan.real_owner_id()));
        assert!(!loan.grants_approval_authority_to(loan.borrowing_manager_id()));
        assert!(!loan.grants_approval_authority_to(test_user_id()));
    }

    #[test]
    fn escalation_target_can_approve_the_loan() {
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let escalation_target = test_user_id();
        let escalate_events = loan
            .decide(
                StaffLoanCommand::EscalateStaffLoan {
                    reason: "Real owner unreachable".to_string(),
                    escalated_to: escalation_target,
                },
                &ctx,
            )
            .expect("escalate must succeed");
        apply_all(&mut loan, &escalate_events);

        // decide()'s own authority check is the generic stub (see this
        // crate's established precedent -- the composing application
        // enforces grants_approval_authority_to before dispatch), so
        // ApproveStaffLoan itself succeeds regardless of actor here;
        // this test confirms the state transition still works normally
        // once escalated, which is the mechanism grants_approval_authority_to
        // is meant to gate access to.
        let approve_events = loan
            .decide(StaffLoanCommand::ApproveStaffLoan, &ctx)
            .expect("approve must still succeed from Requested after escalation");
        apply_all(&mut loan, &approve_events);
        assert_eq!(loan.status(), StaffLoanStatus::Active);
    }

    #[test]
    fn re_escalating_a_staff_loan_overwrites_the_target() {
        let mut loan = requested_staff_loan();
        let ctx = test_context();
        let first_target = test_user_id();
        let first_events = loan
            .decide(
                StaffLoanCommand::EscalateStaffLoan {
                    reason: "First escalation".to_string(),
                    escalated_to: first_target,
                },
                &ctx,
            )
            .expect("first escalate must succeed");
        apply_all(&mut loan, &first_events);
        assert_eq!(loan.escalated_to(), Some(first_target));

        let second_target = test_user_id();
        assert_ne!(first_target, second_target);
        let second_events = loan
            .decide(
                StaffLoanCommand::EscalateStaffLoan {
                    reason: "Still stuck".to_string(),
                    escalated_to: second_target,
                },
                &ctx,
            )
            .expect("re-escalate must succeed from Requested");
        apply_all(&mut loan, &second_events);
        assert_eq!(loan.status(), StaffLoanStatus::Requested);
        assert_eq!(loan.escalated_to(), Some(second_target));
        assert!(!loan.grants_approval_authority_to(first_target));
        assert!(loan.grants_approval_authority_to(second_target));
    }
}

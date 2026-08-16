//! Deterministic test fixtures for `todo-domain`'s test suites.
//!
//! Deliberately self-contained rather than reusing another domain
//! crate's fixtures — same rationale as `policy_domain::test_support`'s
//! doc comment.

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, UserId,
    VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::aggregate::{StaffLoan, TargetList, TodoList};
use crate::command::{StaffLoanCommand, TargetListCommand, TodoListCommand};
use crate::value::{ListOrigin, LoanWindow, TimeWindow, TodoItem};
use platform_contracts::AggregateRoot;

/// A deterministic, counter-based `IdGenerator` for tests. See
/// `mission_domain::test_support::DeterministicIdGenerator`'s doc
/// comment for the full rationale.
struct DeterministicIdGenerator {
    counter: AtomicU64,
}

impl DeterministicIdGenerator {
    fn new_seeded(seed: u64) -> Self {
        Self {
            counter: AtomicU64::new(seed),
        }
    }

    fn next_bytes(&self) -> [u8; 16] {
        let value = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[8..].copy_from_slice(&value.to_be_bytes());
        bytes
    }
}

impl IdGenerator for DeterministicIdGenerator {
    fn generate_object_id(&self) -> ObjectId {
        ObjectId(self.next_bytes())
    }
    fn generate_operation_id(&self) -> OperationId {
        OperationId(self.next_bytes())
    }
    fn generate_event_id(&self) -> EventId {
        EventId(self.next_bytes())
    }
}

/// Process-wide seed counter, same isolation rationale as
/// `policy_domain::test_support::TEST_CONTEXT_SEED`.
static TEST_CONTEXT_SEED: AtomicU64 = AtomicU64::new(1);

/// Builds a `DecisionContext` suitable for deterministic tests, with
/// `trusted_now` fixed at a base instant. Use
/// [`test_context_at`] when a test needs to control the clock
/// explicitly (e.g. target/loan window boundary tests).
pub fn test_context() -> DecisionContext {
    test_context_at(Timestamp::from_nanos(1_700_000_000_000_000_000))
}

/// Builds a `DecisionContext` with `trusted_now` set to a specific
/// instant — needed for `TargetList`'s `WindowNotClosed` boundary tests
/// and `StaffLoan`'s window-comparison tests, where the test must
/// control "now" relative to a window's `start_at`/`end_at`.
pub fn test_context_at(trusted_now: Timestamp) -> DecisionContext {
    let seed = TEST_CONTEXT_SEED.fetch_add(1_000_000, Ordering::SeqCst);
    let org = ObjectId::new_random();
    DecisionContext {
        actor: ActorContext {
            user_id: ObjectId::new_random(),
            device_id: ObjectId::new_random(),
            organization_id: org,
        },
        authority: VerifiedAuthority,
        trusted_now,
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(DeterministicIdGenerator::new_seeded(seed)),
    }
}

/// Generates a fresh random `UserId` for use as an actor in tests.
pub fn test_user_id() -> UserId {
    ObjectId::new_random()
}

/// A single todo item fixture.
pub fn todo_item(description: &str) -> TodoItem {
    TodoItem {
        item_id: ObjectId::new_random().to_string(),
        description: description.to_string(),
    }
}

/// A freshly created, `Draft` todo list, staff-authored, with one item.
pub fn draft_todo_list() -> TodoList {
    let ctx = test_context();
    let owner = ctx.actor.user_id;
    let events = TodoList::create(
        TodoListCommand::CreateTodoList {
            owner,
            origin: ListOrigin::StaffAuthored,
            items: vec![todo_item("Write the weekly report")],
        },
        &ctx,
    )
    .expect("create must succeed");
    TodoList::from_created_event(&events[0])
}

/// A `Submitted` todo list — a `draft_todo_list()` advanced by one
/// transition, the common starting point for verification tests.
pub fn submitted_todo_list() -> TodoList {
    let mut list = draft_todo_list();
    let ctx = test_context();
    let events = list
        .decide(TodoListCommand::SubmitTodoList, &ctx)
        .expect("submit must succeed");
    for event in &events {
        list.apply(event);
    }
    list
}

/// A `TimeWindow` fixture: `start_at` at the base test instant,
/// `end_at` one hour later.
pub fn open_time_window() -> TimeWindow {
    TimeWindow {
        start_at: Timestamp::from_nanos(1_700_000_000_000_000_000),
        end_at: Timestamp::from_nanos(1_700_000_000_000_000_000 + 3_600_000_000_000),
    }
}

/// A freshly created, `Draft` target list with `open_time_window()`.
pub fn draft_target_list() -> TargetList {
    let ctx = test_context();
    let owner = ctx.actor.user_id;
    let events = TargetList::create(
        TargetListCommand::CreateTargetList {
            owner,
            origin: ListOrigin::StaffAuthored,
            description: "Close 10 support tickets this week".to_string(),
            time_window: open_time_window(),
        },
        &ctx,
    )
    .expect("create must succeed");
    TargetList::from_created_event(&events[0])
}

/// A `Submitted` target list, same window as `draft_target_list()`.
pub fn submitted_target_list() -> TargetList {
    let mut list = draft_target_list();
    let ctx = test_context();
    let events = list
        .decide(TargetListCommand::SubmitTargetList, &ctx)
        .expect("submit must succeed");
    for event in &events {
        list.apply(event);
    }
    list
}

/// A `LoanWindow` fixture: `start_at` at the base test instant,
/// `end_at` seven days later.
pub fn two_week_loan_window() -> LoanWindow {
    const NANOS_PER_DAY: u64 = 86_400_000_000_000;
    LoanWindow {
        start_at: Timestamp::from_nanos(1_700_000_000_000_000_000),
        end_at: Timestamp::from_nanos(1_700_000_000_000_000_000 + 7 * NANOS_PER_DAY),
    }
}

/// A freshly requested, `Requested` staff loan between two distinct
/// generated managers.
pub fn requested_staff_loan() -> StaffLoan {
    let ctx = test_context();
    let staff_user_id = test_user_id();
    let real_owner_id = test_user_id();
    let borrowing_manager_id = test_user_id();
    let events = StaffLoan::create(
        StaffLoanCommand::RequestStaffLoan {
            staff_user_id,
            real_owner_id,
            borrowing_manager_id,
            window: two_week_loan_window(),
        },
        &ctx,
    )
    .expect("create must succeed");
    StaffLoan::from_created_event(&events[0])
}

/// An `Active` staff loan — a `requested_staff_loan()` advanced by the
/// real owner's approval.
pub fn active_staff_loan() -> StaffLoan {
    let mut loan = requested_staff_loan();
    let ctx = test_context();
    let events = loan
        .decide(StaffLoanCommand::ApproveStaffLoan, &ctx)
        .expect("approve must succeed");
    for event in &events {
        loan.apply(event);
    }
    loan
}

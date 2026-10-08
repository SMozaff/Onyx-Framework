//! Creation routes for `todo_domain::TodoList`, `TargetList`, and
//! `StaffLoan`. Added 2026-08-16, following exactly the precedent
//! `routes::policy_admin` set for `Policy`/`LegalHold`:
//! `CreateTodoList`/`CreateTargetList`/`RequestStaffLoan` are
//! `create()`-routed commands, not `decide()`-routed ones, so they
//! cannot go through `routes::command`'s `handle_command`-based
//! dispatch the way every other `todo_list.*`/`target_list.*`/
//! `staff_loan.*` command added there does. This module gives them
//! their own REST-shaped entry points instead.
//!
//! # Authorization — deliberately not admin-only
//! Unlike `policy_admin`'s routes (Policy administration is Admin-only
//! per design doc §1), creating a `TodoList`/`TargetList` is confirmed
//! **bidirectional** — either the owning Staff member or a Manager may
//! create one (`DESIGN_User_Hierarchy_Chain_of_Authority.md` §4.0.1) —
//! and requesting a `StaffLoan` is a Manager action with no
//! Admin-specific restriction noted in design doc §2.1. These routes
//! therefore only call `authenticate_headers`, not `require_admin` or
//! `require_class`: any authenticated user may call them. The
//! resulting aggregate's `owner`/`real_owner_id`/`borrowing_manager_id`
//! fields are supplied explicitly in the request body rather than
//! inferred from the caller, since (per design doc §4.0.1) the creator
//! and the owner are not always the same person — a stricter
//! authorization model (e.g. confirming the caller is actually allowed
//! to create *on behalf of* a different `owner`) is exactly the
//! verifier-resolution class of concern flagged as not yet built in
//! `todo_domain`'s own crate docs (D.4) and is not invented here.

use axum::{extract::State, http::HeaderMap, Json};
use platform_contracts::AggregateRoot;
use serde::{Deserialize, Serialize};
use todo_domain::aggregate::{StaffLoan, TargetList, TodoList};
use todo_domain::command::{StaffLoanCommand, TargetListCommand, TodoListCommand};
use todo_domain::value::{ListOrigin, LoanWindow, TimeWindow, TodoItem};

use super::profiles::{decision_context_for_actor, domain_error, infrastructure_error};
use super::{authenticate_headers, ApiState};
use crate::routes::ApiError;

/// Wire form of [`ListOrigin`]. A separate DTO rather than deriving
/// `Deserialize` directly on the domain type, matching this
/// workspace's established boundary between wire DTOs and domain
/// value types elsewhere in `routes::*`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListOriginDto {
    ManagerAssigned,
    StaffAuthored,
}

impl From<ListOriginDto> for ListOrigin {
    fn from(dto: ListOriginDto) -> Self {
        match dto {
            ListOriginDto::ManagerAssigned => ListOrigin::ManagerAssigned,
            ListOriginDto::StaffAuthored => ListOrigin::StaffAuthored,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TodoItemDto {
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTodoListRequest {
    pub owner: String,
    pub origin: ListOriginDto,
    #[serde(default)]
    pub items: Vec<TodoItemDto>,
}

#[derive(Debug, Serialize)]
pub struct CreatedTodoListDto {
    pub todo_list_id: String,
}

/// `POST /api/todo/lists` — creates a new `TodoList`. See this module's
/// doc comment for why this is not admin-gated.
pub async fn create_todo_list(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreateTodoListRequest>,
) -> Result<Json<CreatedTodoListDto>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    // H10/P1.4: TodoList/TargetList/StaffLoan creation is a real domain
    // command submission (see this module's own doc comment on why
    // these are `create()`-routed here rather than through
    // `/api/command`'s `handle_command` dispatch) -- gated the same way
    // as every other domain command, `command_route`'s included.
    super::client_type::require_capability(
        &auth,
        |c| c.can_submit_domain_commands,
        "submit_domain_command",
    )?;
    let organization_id = uuid::Uuid::parse_str(&auth.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let ctx = decision_context_for_actor(&auth.user_id, organization_id)?;

    let owner_uuid =
        uuid::Uuid::parse_str(&req.owner).map_err(|_| domain_error("owner is not a valid UUID"))?;

    let events = TodoList::create(
        TodoListCommand::CreateTodoList {
            owner: platform_kernel::ObjectId(*owner_uuid.as_bytes()),
            origin: req.origin.into(),
            items: req
                .items
                .into_iter()
                .map(|item| TodoItem {
                    item_id: uuid::Uuid::new_v4().to_string(),
                    description: item.description,
                })
                .collect(),
        },
        &ctx,
    )
    .map_err(domain_error)?;
    let list = TodoList::from_created_event(&events[0]);
    let aggregate_state =
        serde_json::to_value(&list).map_err(|e| infrastructure_error(e.to_string()))?;

    let mut unit = state
        .unit_factory
        .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    state
        .todo_list_repo
        .commit(aggregate_state, &[], &mut *unit)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    unit.commit()
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;

    Ok(Json(CreatedTodoListDto {
        todo_list_id: list.id().0.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct TimeWindowDto {
    /// Milliseconds since the Unix epoch.
    pub start_at_ms: u64,
    /// Milliseconds since the Unix epoch.
    pub end_at_ms: u64,
}

impl From<TimeWindowDto> for TimeWindow {
    fn from(dto: TimeWindowDto) -> Self {
        TimeWindow {
            start_at: platform_kernel::Timestamp::from_nanos(
                dto.start_at_ms.saturating_mul(1_000_000),
            ),
            end_at: platform_kernel::Timestamp::from_nanos(dto.end_at_ms.saturating_mul(1_000_000)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTargetListRequest {
    pub owner: String,
    pub origin: ListOriginDto,
    pub description: String,
    pub time_window: TimeWindowDto,
}

#[derive(Debug, Serialize)]
pub struct CreatedTargetListDto {
    pub target_list_id: String,
}

/// `POST /api/todo/targets` — creates a new `TargetList`. Same
/// authorization rationale as `create_todo_list` above.
pub async fn create_target_list(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreateTargetListRequest>,
) -> Result<Json<CreatedTargetListDto>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    // H10/P1.4: TodoList/TargetList/StaffLoan creation is a real domain
    // command submission (see this module's own doc comment on why
    // these are `create()`-routed here rather than through
    // `/api/command`'s `handle_command` dispatch) -- gated the same way
    // as every other domain command, `command_route`'s included.
    super::client_type::require_capability(
        &auth,
        |c| c.can_submit_domain_commands,
        "submit_domain_command",
    )?;
    let organization_id = uuid::Uuid::parse_str(&auth.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let ctx = decision_context_for_actor(&auth.user_id, organization_id)?;

    let owner_uuid =
        uuid::Uuid::parse_str(&req.owner).map_err(|_| domain_error("owner is not a valid UUID"))?;

    let events = TargetList::create(
        TargetListCommand::CreateTargetList {
            owner: platform_kernel::ObjectId(*owner_uuid.as_bytes()),
            origin: req.origin.into(),
            description: req.description,
            time_window: req.time_window.into(),
        },
        &ctx,
    )
    .map_err(domain_error)?;
    let list = TargetList::from_created_event(&events[0]);
    let aggregate_state =
        serde_json::to_value(&list).map_err(|e| infrastructure_error(e.to_string()))?;

    let mut unit = state
        .unit_factory
        .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    state
        .target_list_repo
        .commit(aggregate_state, &[], &mut *unit)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    unit.commit()
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;

    Ok(Json(CreatedTargetListDto {
        target_list_id: list.id().0.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct RequestStaffLoanRequest {
    pub staff_user_id: String,
    pub real_owner_id: String,
    pub borrowing_manager_id: String,
    /// Milliseconds since the Unix epoch.
    pub start_at_ms: u64,
    /// Milliseconds since the Unix epoch.
    pub end_at_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct CreatedStaffLoanDto {
    pub staff_loan_id: String,
}

/// `POST /api/todo/staff-loans` — requests a new `StaffLoan`. Starts in
/// `Requested`, awaiting the real owner's approval via
/// `staff_loan.ApproveStaffLoan` over `/api/command` (design doc §2.1).
/// Same authorization rationale as `create_todo_list` above — any
/// authenticated user may request a loan; the real owner's approval is
/// the actual gate, enforced at the next step, not here.
pub async fn request_staff_loan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<RequestStaffLoanRequest>,
) -> Result<Json<CreatedStaffLoanDto>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    // H10/P1.4: TodoList/TargetList/StaffLoan creation is a real domain
    // command submission (see this module's own doc comment on why
    // these are `create()`-routed here rather than through
    // `/api/command`'s `handle_command` dispatch) -- gated the same way
    // as every other domain command, `command_route`'s included.
    super::client_type::require_capability(
        &auth,
        |c| c.can_submit_domain_commands,
        "submit_domain_command",
    )?;
    let organization_id = uuid::Uuid::parse_str(&auth.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let ctx = decision_context_for_actor(&auth.user_id, organization_id)?;

    let staff_user_id = uuid::Uuid::parse_str(&req.staff_user_id)
        .map_err(|_| domain_error("staff_user_id is not a valid UUID"))?;
    let real_owner_id = uuid::Uuid::parse_str(&req.real_owner_id)
        .map_err(|_| domain_error("real_owner_id is not a valid UUID"))?;
    let borrowing_manager_id = uuid::Uuid::parse_str(&req.borrowing_manager_id)
        .map_err(|_| domain_error("borrowing_manager_id is not a valid UUID"))?;

    let events = StaffLoan::create(
        StaffLoanCommand::RequestStaffLoan {
            staff_user_id: platform_kernel::ObjectId(*staff_user_id.as_bytes()),
            real_owner_id: platform_kernel::ObjectId(*real_owner_id.as_bytes()),
            borrowing_manager_id: platform_kernel::ObjectId(*borrowing_manager_id.as_bytes()),
            window: LoanWindow {
                start_at: platform_kernel::Timestamp::from_nanos(
                    req.start_at_ms.saturating_mul(1_000_000),
                ),
                end_at: platform_kernel::Timestamp::from_nanos(
                    req.end_at_ms.saturating_mul(1_000_000),
                ),
            },
        },
        &ctx,
    )
    .map_err(domain_error)?;
    let loan = StaffLoan::from_created_event(&events[0]);
    let aggregate_state =
        serde_json::to_value(&loan).map_err(|e| infrastructure_error(e.to_string()))?;

    let mut unit = state
        .unit_factory
        .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    state
        .staff_loan_repo
        .commit(aggregate_state, &[], &mut *unit)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    unit.commit()
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;

    Ok(Json(CreatedStaffLoanDto {
        staff_loan_id: loan.id().0.to_string(),
    }))
}

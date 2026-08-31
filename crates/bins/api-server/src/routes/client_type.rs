//! Server-owned client classification and capability ceiling.
//!
//! # Provenance — H10 / ONYX-MOB-00 v1.1 + ONYX-MOB-01 v1.1
//! Both governance documents describe `client_type` as if a closed,
//! rejecting enum already existed. It did not: before this module,
//! `LoginRequest::client_type` was a loose `Option<String>`, checked
//! with exactly one hardcoded string comparison
//! (`payload.client_type.as_deref() == Some("mobile")`), and nothing
//! rejected an unrecognized value. This module makes the type real —
//! see `DECISIONS.md`'s H10 entry for the full corrected wording this
//! task pushed back into both governance documents.
//!
//! # Reversed tradeoff — read this before touching the compatibility default
//! `LoginRequest::client_type`'s old doc comment explained a real,
//! deliberate decision: treat a missing `client_type` as `None`, which
//! was never gated, "so any caller this project doesn't yet know about
//! is not silently locked out." That reasoning does not disappear here
//! — it is why this module still treats an *absent* `client_type` as a
//! non-observer classification (see [`ClientType::default_on_absence`])
//! rather than hard-requiring the field at login. What changes is that
//! an *unrecognized* value is no longer silently ignored: `serde`'s
//! generated `Deserialize` for a plain string-valued enum already
//! rejects any string that isn't one of the five known variants (calls
//! `serde::de::Error::unknown_variant` — confirmed against current
//! serde documentation via Context7 before writing this, not assumed),
//! so `ClientType`'s definition alone satisfies the "reject unknown
//! client types" requirement without any hand-written validation.
//!
//! The genuinely new security boundary this module adds is
//! `MobileObserver`'s capability ceiling (below), which the old
//! permissive design never had a slot for at all: there was no
//! observer-class client, and no code path anywhere in this codebase
//! ever denied a mutation on the basis of *what kind of client* sent
//! it. That boundary cannot be optional/permissive by design — a
//! `mobile_observer` session that could sometimes still mutate would
//! not be a security boundary, it would be a suggestion. The
//! resolution is that these are two separate concerns: the *shape* of
//! the field stays additive/back-compat (per the original reasoning,
//! and per the real internal callers confirmed still to omit it — see
//! `DECISIONS.md`), but the *capability ceiling* for whichever type is
//! ultimately resolved is absolute and server-enforced, never bypassed
//! by a client-declared value the server didn't already accept.
use serde::{Deserialize, Serialize};

use super::{ApiError, AuthenticatedUser};

/// The closed, server-owned set of recognized client classes.
///
/// `#[serde(rename_all = "snake_case")]` renders `MobileObserver` as
/// `"mobile_observer"` and every other variant as its lowercase name —
/// confirmed to match every real client's current login call site
/// exactly (`"mobile"`: `mobile/lib/net/auth.dart`; `"desktop"`:
/// `desktop-shell/src/session.rs`; `"admin"`:
/// `admin-shell/ui/src/pages/Login.tsx`; `"web"`:
/// `web-ui/src/hooks/useAuth.ts`), not assumed from the rename attribute
/// alone.
///
/// A plain string-valued enum deriving `Deserialize` already rejects any
/// string outside this list (`serde::de::Error::unknown_variant`), so
/// no separate "reject unknown client types" logic is needed —
/// `LoginRequest::client_type: Option<ClientType>` gets that for free
/// the moment a request supplies a `client_type` value serde doesn't
/// recognize. A field that's *absent* entirely is a different case,
/// handled by [`ClientType::default_on_absence`] below, not by this
/// rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Mobile,
    MobileObserver,
    Desktop,
    Admin,
    Web,
}

impl ClientType {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientType::Mobile => "mobile",
            ClientType::MobileObserver => "mobile_observer",
            ClientType::Desktop => "desktop",
            ClientType::Admin => "admin",
            ClientType::Web => "web",
        }
    }

    /// The compatibility-policy decision for a login request (or an
    /// already-issued token predating this field) that carries no
    /// `client_type` at all.
    ///
    /// # Real decision, not an assumption
    /// Confirmed by grepping every real internal caller before making
    /// this decision (`crates/bins/api-server/tests/*.rs`,
    /// `tests/end-to-end/*.rs`, `tests/integration/*.rs`): dozens of
    /// this project's own test files -- including `test_harness.rs`,
    /// shared by every end-to-end journey -- call `/api/auth/login`
    /// without ever sending `client_type`. Requiring the field outright
    /// (rejecting a login that omits it) would therefore break a large
    /// number of real, existing internal callers, not just a
    /// hypothetical external one. That rules out "require it outright"
    /// as unsafe today, despite every *first-party product* client
    /// already sending a real value.
    ///
    /// So: an absent `client_type` resolves to `Web`, not a new
    /// `Unknown`-flavored sixth variant. `Web` was picked deliberately
    /// over inventing another variant, because (a) the governance
    /// documents define exactly five real client classes and adding a
    /// sixth "legacy/unclassified" variant to carry this one fallback
    /// case would widen what's supposed to be a small, closed enum for
    /// a purely internal bookkeeping reason, and (b) `Web` already
    /// carries full (non-observer) capabilities identical to what every
    /// caller that omits the field has always implicitly had under the
    /// old "`None` is never gated" behavior -- so this default is a
    /// literal continuation of the pre-H10 default, not a new policy.
    ///
    /// The one case this default must never cover is a caller
    /// *explicitly* declaring `"mobile_observer"`: that always resolves
    /// to the real `MobileObserver` capability ceiling, never this
    /// fallback, because the field was actually present and recognized.
    pub fn default_on_absence() -> ClientType {
        ClientType::Web
    }
}

/// The server-owned capability ceiling for a client class.
///
/// # Scope decision
/// This project has no pre-existing unified `user_permissions` concept
/// to intersect against (`ONYX_Mobile_Client_Strategy_Manifesto`'s
/// `effective_permissions(user, session) = user_permissions(user) ∩
/// observer_capabilities(session.client_type)`, literally) -- authority
/// today is checked ad hoc per route (`require_admin`,
/// `verifier_resolution`, H2's revocation watermark, H7's relay
/// ownership, etc.), not through one permissions object. Retroactively
/// unifying all of that into one real permissions type is a much
/// larger, riskier change than this task's actual scope (closing the
/// `MobileObserver` boundary) calls for, and is deliberately not
/// attempted here. What this module builds instead is the minimal real
/// intersection this task needs: every mutation-class endpoint checks
/// `client_capabilities(session.client_type).can_<X>` and denies if
/// false, regardless of what the authenticated user could otherwise do
/// -- which is exactly what "an administrator using `mobile_observer`
/// still cannot mutate" requires, without touching any of the existing
/// per-route authority mechanisms above. A user who fails their
/// existing authority check is still denied by that check first; this
/// ceiling only ever narrows further, never substitutes for it.
///
/// `can_read_evidence`/`can_download_files` are flat bools here, not a
/// policy-object hook, per ONYX-MOB-01 §8's own "policy-controlled"
/// caveat -- confirmed no real per-file/per-evidence authorization
/// policy engine exists in this codebase to hook into (object-level
/// tenant/ownership checks already gate individual reads elsewhere;
/// this flag is only the client-*class* ceiling on top of those, not a
/// new authorization layer). If a real per-file/per-evidence policy
/// engine is built later, revisit these two fields then -- the
/// simplest static mapping was chosen because nothing today proves it
/// insufficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientCapabilities {
    pub can_read_projections: bool,
    pub can_read_notifications: bool,
    pub can_read_evidence: bool,
    pub can_download_files: bool,
    pub can_submit_domain_commands: bool,
    pub can_approve: bool,
    pub can_transition_lifecycle: bool,
    pub can_resolve_conflicts: bool,
    pub can_upload_files: bool,
    pub can_administer: bool,
}

const FULL_CAPABILITIES: ClientCapabilities = ClientCapabilities {
    can_read_projections: true,
    can_read_notifications: true,
    can_read_evidence: true,
    can_download_files: true,
    can_submit_domain_commands: true,
    can_approve: true,
    can_transition_lifecycle: true,
    can_resolve_conflicts: true,
    can_upload_files: true,
    can_administer: true,
};

/// `MobileObserver`'s ceiling, exactly per ONYX-MOB-00 §17 / ONYX-MOB-01
/// §8: every `can_read_*`/`can_download_files` true, every
/// mutation-capable flag false. `can_download_files` being true does
/// not itself authorize any specific download -- real per-file
/// authorization (already enforced independently elsewhere) still
/// applies to the actual file; this flag only says the client *class*
/// is not blanket-forbidden from ever downloading.
const OBSERVER_CAPABILITIES: ClientCapabilities = ClientCapabilities {
    can_read_projections: true,
    can_read_notifications: true,
    can_read_evidence: true,
    can_download_files: true,
    can_submit_domain_commands: false,
    can_approve: false,
    can_transition_lifecycle: false,
    can_resolve_conflicts: false,
    can_upload_files: false,
    can_administer: false,
};

/// The server-owned mapping from client class to capability ceiling.
///
/// A `const fn`/static mapping, per ONYX-MOB-01 §8's own "the exact
/// implementation may be enum/bitset/typed policy object" language --
/// the simplest option that satisfies today's requirement, not ruled
/// insufficient by anything in this task's real scope.
pub const fn capabilities_for(client_type: ClientType) -> ClientCapabilities {
    match client_type {
        ClientType::MobileObserver => OBSERVER_CAPABILITIES,
        ClientType::Mobile | ClientType::Desktop | ClientType::Admin | ClientType::Web => {
            FULL_CAPABILITIES
        }
    }
}

/// Denies a request unless the authenticated session's client class
/// grants `required`. `check` selects which [`ClientCapabilities`]
/// field the caller needs; `required_capability` is the wire name
/// reported back to the client on denial (matching the field name
/// minus its `can_` prefix, e.g. `"submit_domain_command"`,
/// `"administer"`).
///
/// # Error shape
/// ONYX-MOB-01 §9 gives an illustrative shape
/// (`{"error": "ClientCapabilityDenied", "client_type": ...,
/// "required_capability": ...}`) but explicitly defers to this
/// project's real conventions ("Exact field names must align with ONYX
/// error conventions"). This project's real convention (H2/H7's error
/// work) is `ApiError`/`ApiErrorBody`: a `code`, `category`,
/// `retryability`, a `correlation_id`, and a `safe_details` payload for
/// anything response-specific. `client_type` and `required_capability`
/// therefore live inside `safe_details`, with `code:
/// "CLIENT_CAPABILITY_DENIED"` as the deterministic discriminator the
/// blueprint calls for -- not the blueprint's flat example shape
/// verbatim.
pub fn require_capability(
    auth: &AuthenticatedUser,
    check: impl Fn(&ClientCapabilities) -> bool,
    required_capability: &str,
) -> Result<(), ApiError> {
    let capabilities = capabilities_for(auth.client_type);
    if check(&capabilities) {
        return Ok(());
    }
    tracing::warn!(
        user_id = %auth.user_id,
        organization_id = %auth.organization_id,
        client_type = auth.client_type.as_str(),
        required_capability,
        "client capability denied: session's client class does not permit this mutation"
    );
    Err(ApiError::new(
        axum::http::StatusCode::FORBIDDEN,
        "CLIENT_CAPABILITY_DENIED",
        "AUTHORITY",
        "NON_RETRYABLE",
        uuid::Uuid::new_v4().to_string(),
        serde_json::json!({
            "client_type": auth.client_type.as_str(),
            "required_capability": required_capability,
        }),
    ))
}

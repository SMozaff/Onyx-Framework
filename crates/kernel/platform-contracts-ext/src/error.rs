//! The domain error protocol shared by every aggregate.
//!
//! # Serialization Ruling
//! `DomainError` serializes internally-tagged as `{"code": ..., "safe_details":
//! {...}}` via `#[serde(tag = "code", content = "safe_details")]`. The full
//! wire shape required by the Handover Document error protocol — `code`,
//! `category`, `retryability`, `safe_details`, `correlation_id` — is produced
//! by [`DomainErrorResponse`], an API-layer wrapper. `DomainError` itself
//! carries no `correlation_id`; that is attached by the command handler
//! (Increment 2) once a `CorrelationId` is available. See ruling E1.

use platform_kernel::{ConflictId, CorrelationId, LifecycleEpoch, ObjectId, ObjectVersion};
use serde::Serialize;

/// A domain-level error, produced by an aggregate's `decide()` method.
///
/// Serializes internally-tagged by `code` with a `safe_details` payload
/// (ruling E1). No stack traces, no secrets, no raw policy expressions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "code", content = "safe_details")]
pub enum DomainError {
    /// The command is not valid from the aggregate's current state.
    #[error("Invalid transition from {from:?} for command {command}")]
    #[serde(rename = "INVALID_TRANSITION")]
    InvalidTransition {
        /// The state the aggregate was in when the command was rejected.
        from: String,
        /// The command type that was rejected.
        command: String,
    },

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: actor lacks required authority")]
    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,

    /// The command's expected version does not match the aggregate's
    /// actual version (optimistic concurrency conflict).
    #[error("Version conflict: expected {expected:?}, actual {actual:?}")]
    #[serde(rename = "VERSION_CONFLICT")]
    VersionConflict {
        /// The version the caller expected.
        expected: ObjectVersion,
        /// The aggregate's actual current version.
        actual: ObjectVersion,
    },

    /// The command's expected lifecycle epoch does not match the
    /// aggregate's actual lifecycle epoch.
    #[error("Epoch conflict: expected {expected:?}, actual {actual:?}")]
    #[serde(rename = "EPOCH_CONFLICT")]
    EpochConflict {
        /// The lifecycle epoch the caller expected.
        expected: LifecycleEpoch,
        /// The aggregate's actual current lifecycle epoch.
        actual: LifecycleEpoch,
    },

    /// The referenced object does not exist.
    #[error("Object not found: {id:?}")]
    #[serde(rename = "NOT_FOUND")]
    NotFound {
        /// The identifier that could not be resolved.
        id: ObjectId,
    },

    /// An object with this identity already exists.
    #[error("Object already exists: {id:?}")]
    #[serde(rename = "ALREADY_EXISTS")]
    AlreadyExists {
        /// The identifier that already exists.
        id: ObjectId,
    },

    /// A supplied argument failed validation.
    #[error("Invalid argument: {field} - {reason}")]
    #[serde(rename = "INVALID_ARGUMENT")]
    InvalidArgument {
        /// The name of the invalid field.
        field: String,
        /// A human-readable explanation of why it is invalid.
        reason: String,
    },

    /// The aggregate has an unresolved conflict and cannot be mutated.
    ///
    /// # Increment 1 Ruling
    /// `ConflictId` is a placeholder type. No Increment 1 domain code
    /// constructs this variant; it exists for forward wire-compatibility
    /// with Increment 3.
    #[error("Conflict pending: {conflict_id:?} - cannot mutate while conflict unresolved")]
    #[serde(rename = "CONFLICT_PENDING")]
    ConflictPending {
        /// The identifier of the unresolved conflict.
        conflict_id: ConflictId,
    },
}

impl DomainError {
    /// The stable machine-readable error code (matches the `code` field
    /// this type serializes to).
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::InvalidTransition { .. } => "INVALID_TRANSITION",
            DomainError::Unauthorized => "UNAUTHORIZED",
            DomainError::VersionConflict { .. } => "VERSION_CONFLICT",
            DomainError::EpochConflict { .. } => "EPOCH_CONFLICT",
            DomainError::NotFound { .. } => "NOT_FOUND",
            DomainError::AlreadyExists { .. } => "ALREADY_EXISTS",
            DomainError::InvalidArgument { .. } => "INVALID_ARGUMENT",
            DomainError::ConflictPending { .. } => "CONFLICT_PENDING",
        }
    }

    /// The error category, per the Handover Document error protocol.
    pub fn category(&self) -> &'static str {
        match self {
            DomainError::InvalidTransition { .. } => "DOMAIN",
            DomainError::Unauthorized => "AUTHORITY",
            DomainError::VersionConflict { .. } => "CONCURRENCY",
            DomainError::EpochConflict { .. } => "CONCURRENCY",
            DomainError::NotFound { .. } => "DOMAIN",
            DomainError::AlreadyExists { .. } => "DOMAIN",
            DomainError::InvalidArgument { .. } => "DOMAIN",
            DomainError::ConflictPending { .. } => "DOMAIN",
        }
    }

    /// Whether a caller may retry the same command after this error,
    /// per the ruling's retryability mapping.
    pub fn retryability(&self) -> &'static str {
        match self {
            DomainError::InvalidTransition { .. } => "NON_RETRYABLE",
            DomainError::Unauthorized => "NON_RETRYABLE",
            DomainError::VersionConflict { .. } => "RETRYABLE",
            DomainError::EpochConflict { .. } => "RETRYABLE",
            DomainError::NotFound { .. } => "NON_RETRYABLE",
            DomainError::AlreadyExists { .. } => "NON_RETRYABLE",
            DomainError::InvalidArgument { .. } => "NON_RETRYABLE",
            DomainError::ConflictPending { .. } => "RETRYABLE",
        }
    }

    /// The `safe_details` payload: this error's fields, serialized as JSON,
    /// with secrets/stack traces never present by construction (the enum
    /// carries no such fields).
    pub fn safe_details(&self) -> serde_json::Value {
        // `DomainError` serializes as {"code": ..., "safe_details": ...}
        // (via `tag`/`content`), so extracting the `safe_details` key from
        // its own serialization gives us exactly this error's field payload
        // (or `Null` for unit variants such as `Unauthorized`).
        let full = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        full.get("safe_details")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
}

/// The API-layer wire shape for a domain error, per the Handover Document
/// error protocol: `{"code", "category", "retryability", "safe_details",
/// "correlation_id"}`.
///
/// # Increment 1 Ruling (E1)
/// `DomainError` does not know about the request that caused it, so
/// `correlation_id` is attached here, at the boundary, not on `DomainError`
/// itself. Increment 2's command handler constructs this wrapper once a
/// `CorrelationId` is available. Golden fixture tests apply to this type,
/// not to `DomainError` directly.
#[derive(Debug, Clone, Serialize)]
pub struct DomainErrorResponse {
    /// The stable machine-readable error code.
    pub code: String,
    /// The error category (e.g. `"DOMAIN"`, `"AUTHORITY"`, `"CONCURRENCY"`).
    pub category: String,
    /// Whether the caller may retry: `"RETRYABLE"`, `"NON_RETRYABLE"`, or
    /// `"TRANSIENT"`.
    pub retryability: String,
    /// Error-specific, secret-free detail fields.
    pub safe_details: serde_json::Value,
    /// The correlation identifier of the request that produced this error.
    pub correlation_id: CorrelationId,
}

impl DomainErrorResponse {
    /// Builds the wire-shape response from a `DomainError` and the
    /// correlation id of the request that produced it.
    pub fn from_error(error: &DomainError, correlation_id: CorrelationId) -> Self {
        Self {
            code: error.code().to_string(),
            category: error.category().to_string(),
            retryability: error.retryability().to_string(),
            safe_details: error.safe_details(),
            correlation_id,
        }
    }
}

/// A stub for the future Repository error type (Increment 2). Present here
/// only so downstream crates can reference a stable path; no domain code in
/// Increment 1 constructs or handles this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
pub enum RepositoryError {
    /// Placeholder variant. Full persistence error taxonomy lands in
    /// Increment 2.
    #[error("repository error (stub): {0}")]
    Stub(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_error_serializes_with_code_tag() {
        let err = DomainError::InvalidTransition {
            from: "Active".to_string(),
            command: "PauseMission".to_string(),
        };
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["code"], "INVALID_TRANSITION");
        assert_eq!(value["safe_details"]["from"], "Active");
        assert_eq!(value["safe_details"]["command"], "PauseMission");
    }

    #[test]
    fn domain_error_code_matches_serialized_tag() {
        let err = DomainError::Unauthorized;
        assert_eq!(err.code(), "UNAUTHORIZED");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["code"], "UNAUTHORIZED");
    }

    #[test]
    fn domain_error_response_matches_handover_wire_shape() {
        let err = DomainError::VersionConflict {
            expected: ObjectVersion(1),
            actual: ObjectVersion(2),
        };
        let correlation_id = CorrelationId::new_random();
        let response = DomainErrorResponse::from_error(&err, correlation_id);
        assert_eq!(response.code, "VERSION_CONFLICT");
        assert_eq!(response.category, "CONCURRENCY");
        assert_eq!(response.retryability, "RETRYABLE");

        let value = serde_json::to_value(&response).unwrap();
        assert!(value.get("code").is_some());
        assert!(value.get("category").is_some());
        assert!(value.get("retryability").is_some());
        assert!(value.get("safe_details").is_some());
        assert!(value.get("correlation_id").is_some());
    }

    #[test]
    fn retryability_mapping_matches_ruling_table() {
        assert_eq!(
            DomainError::InvalidTransition {
                from: "X".into(),
                command: "Y".into()
            }
            .retryability(),
            "NON_RETRYABLE"
        );
        assert_eq!(DomainError::Unauthorized.retryability(), "NON_RETRYABLE");
        assert_eq!(
            DomainError::VersionConflict {
                expected: ObjectVersion(0),
                actual: ObjectVersion(1)
            }
            .retryability(),
            "RETRYABLE"
        );
        assert_eq!(
            DomainError::EpochConflict {
                expected: LifecycleEpoch(0),
                actual: LifecycleEpoch(1)
            }
            .retryability(),
            "RETRYABLE"
        );
        assert_eq!(
            DomainError::NotFound {
                id: ObjectId([0; 16])
            }
            .retryability(),
            "NON_RETRYABLE"
        );
        assert_eq!(
            DomainError::AlreadyExists {
                id: ObjectId([0; 16])
            }
            .retryability(),
            "NON_RETRYABLE"
        );
        assert_eq!(
            DomainError::InvalidArgument {
                field: "x".into(),
                reason: "y".into()
            }
            .retryability(),
            "NON_RETRYABLE"
        );
        assert_eq!(
            DomainError::ConflictPending {
                conflict_id: ConflictId([0; 16])
            }
            .retryability(),
            "RETRYABLE"
        );
    }
}

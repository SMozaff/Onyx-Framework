use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityDigest(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityVerification {
    pub valid: bool,
    pub checked_entries: u64,
    pub first_invalid_sequence: Option<i64>,
    pub expected_hash: Option<IntegrityDigest>,
    pub actual_hash: Option<IntegrityDigest>,
}

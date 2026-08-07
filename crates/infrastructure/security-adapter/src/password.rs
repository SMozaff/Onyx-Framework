//! Argon2id password hashing and constant-time verification.
//!
//! # Provenance
//! Audit finding **H-01**. The delivered `api-server` compared credentials as
//! `payload.password != DEFAULT_PASSWORD`, which is (a) a plaintext comparison
//! against a compile-time constant and (b) short-circuiting, so it leaks the
//! length of the matching prefix through timing.
//!
//! # Design notes
//! * **Argon2id**, not Argon2i/Argon2d — it is the OWASP-recommended default
//!   because it resists both side-channel and GPU-cracking attacks.
//! * Hashes are stored in **PHC string format**, so the cost parameters live
//!   inside the hash. Raising the parameters later does not invalidate stored
//!   credentials; old hashes keep verifying with the parameters they were
//!   created with.
//! * [`PasswordHasher::verify_dummy`] exists so the login path can spend the
//!   same CPU time on an unknown username as on a known one. Without it, an
//!   attacker can enumerate valid usernames by measuring response latency —
//!   the exact class of leak this module is meant to close.

use argon2::{Algorithm, Argon2, Params, Version};
use password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString,
};
use subtle::ConstantTimeEq;

/// A precomputed Argon2id hash of a fixed throwaway string, used only by
/// [`PasswordHasher::verify_dummy`]. Generated at first use rather than
/// hardcoded so no real-looking credential material ships in the binary.
const DUMMY_PASSWORD: &str = "onyx-dummy-verification-password";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    #[error("password hashing failed: {0}")]
    Hashing(String),
    #[error("stored password hash is malformed: {0}")]
    MalformedHash(String),
    #[error("password does not satisfy policy: {0}")]
    Policy(String),
}

/// Minimum length enforced on new passwords.
///
/// NIST SP 800-63B recommends length as the primary strength control and
/// explicitly discourages composition rules (forced symbols/digits), which
/// push users toward predictable patterns. So this checks length only.
pub const MIN_PASSWORD_LENGTH: usize = 12;
/// Upper bound. Argon2 itself is unbounded, but accepting unbounded input
/// makes the login endpoint a cheap CPU-exhaustion vector.
pub const MAX_PASSWORD_LENGTH: usize = 1024;

#[derive(Clone)]
pub struct PasswordHasher {
    argon2: Argon2<'static>,
    dummy_hash: String,
}

impl std::fmt::Debug for PasswordHasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never render the dummy hash or parameters into logs.
        f.write_str("PasswordHasher")
    }
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordHasher {
    /// Builds a hasher with OWASP's recommended Argon2id parameters
    /// (19 MiB memory, 2 iterations, 1 degree of parallelism).
    pub fn new() -> Self {
        let params = Params::new(19 * 1024, 2, 1, None)
            .expect("OWASP-recommended Argon2id parameters are valid by construction");
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let salt = SaltString::generate(&mut OsRng);
        let dummy_hash = argon2
            .hash_password(DUMMY_PASSWORD.as_bytes(), &salt)
            .expect("hashing a fixed non-empty password with valid params cannot fail")
            .to_string();
        Self { argon2, dummy_hash }
    }

    /// Validates against the length policy and returns a PHC-format hash.
    pub fn hash(&self, password: &str) -> Result<String, PasswordError> {
        Self::check_policy(password)?;
        let salt = SaltString::generate(&mut OsRng);
        self.argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|error| PasswordError::Hashing(error.to_string()))
    }

    pub fn check_policy(password: &str) -> Result<(), PasswordError> {
        // Count characters, not bytes: a 12-character non-ASCII password is
        // policy-compliant even though its byte length differs.
        let length = password.chars().count();
        if length < MIN_PASSWORD_LENGTH {
            return Err(PasswordError::Policy(format!(
                "password must be at least {MIN_PASSWORD_LENGTH} characters"
            )));
        }
        if password.len() > MAX_PASSWORD_LENGTH {
            return Err(PasswordError::Policy(format!(
                "password must be at most {MAX_PASSWORD_LENGTH} bytes"
            )));
        }
        Ok(())
    }

    /// Verifies a candidate password against a stored PHC hash.
    ///
    /// Returns `Ok(false)` for a mismatch and only errors when the *stored*
    /// hash cannot be parsed — a genuine data-integrity fault that must not be
    /// silently reported as "wrong password".
    pub fn verify(&self, password: &str, stored_hash: &str) -> Result<bool, PasswordError> {
        let parsed = PasswordHash::new(stored_hash)
            .map_err(|error| PasswordError::MalformedHash(error.to_string()))?;
        Ok(self
            .argon2
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    /// Performs a verification against a throwaway hash and discards the
    /// result. Called on the unknown-username path so that path costs the same
    /// as the known-username path, defeating timing-based user enumeration.
    pub fn verify_dummy(&self, password: &str) {
        let _ = self.verify(password, &self.dummy_hash);
    }
}

/// Constant-time string comparison.
///
/// Used for the username check. `==` on `str` short-circuits at the first
/// differing byte; this does not.
pub fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    // Lengths are compared in the clear because the length of a submitted
    // username is already observable from the request body size.
    if left.len() != right.len() {
        return false;
    }
    left.ct_eq(right).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_phc_format_and_verify() {
        let hasher = PasswordHasher::new();
        let hash = hasher.hash("correct horse battery").expect("policy-compliant");
        assert!(hash.starts_with("$argon2id$"), "must be PHC Argon2id: {hash}");
        assert!(hasher.verify("correct horse battery", &hash).unwrap());
        assert!(!hasher.verify("wrong horse battery!!", &hash).unwrap());
    }

    #[test]
    fn same_password_produces_different_hashes() {
        // Proves a per-hash random salt is in use; identical hashes would mean
        // the store is vulnerable to precomputation across users.
        let hasher = PasswordHasher::new();
        let first = hasher.hash("correct horse battery").unwrap();
        let second = hasher.hash("correct horse battery").unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn rejects_short_passwords_and_accepts_boundary() {
        let hasher = PasswordHasher::new();
        assert!(matches!(
            hasher.hash(&"a".repeat(MIN_PASSWORD_LENGTH - 1)),
            Err(PasswordError::Policy(_))
        ));
        assert!(hasher.hash(&"a".repeat(MIN_PASSWORD_LENGTH)).is_ok());
    }

    #[test]
    fn rejects_oversized_passwords() {
        let hasher = PasswordHasher::new();
        assert!(matches!(
            hasher.hash(&"a".repeat(MAX_PASSWORD_LENGTH + 1)),
            Err(PasswordError::Policy(_))
        ));
    }

    #[test]
    fn policy_counts_characters_not_bytes() {
        // 12 multi-byte characters is compliant even though byte length is 36.
        assert!(PasswordHasher::check_policy(&"é".repeat(MIN_PASSWORD_LENGTH)).is_ok());
    }

    #[test]
    fn malformed_stored_hash_is_an_error_not_a_mismatch() {
        let hasher = PasswordHasher::new();
        assert!(matches!(
            hasher.verify("correct horse battery", "not-a-phc-hash"),
            Err(PasswordError::MalformedHash(_))
        ));
    }

    #[test]
    fn dummy_verification_does_not_panic() {
        PasswordHasher::new().verify_dummy("anything at all");
    }

    #[test]
    fn constant_time_eq_matches_semantics_of_equality() {
        assert!(constant_time_eq("operator", "operator"));
        assert!(!constant_time_eq("operator", "operatoR"));
        assert!(!constant_time_eq("operator", "operator2"));
        assert!(constant_time_eq("", ""));
    }
}

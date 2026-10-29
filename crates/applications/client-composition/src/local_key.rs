//! Local database key derivation (Argon2id).
//!
//! The key is derived from the person's password and a per-user random salt.
//! The password itself is never stored. The salt is not secret: it is stored
//! in the person's own data folder so the same key can be derived again at
//! the next sign-in.

use argon2::{Algorithm, Argon2, Params, Version};

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;

/// OWASP minimum for Argon2id: 19 MiB memory, 2 passes, 1 lane.
const MEMORY_KIB: u32 = 19 * 1024;
const PASSES: u32 = 2;
const LANES: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum LocalKeyError {
    #[error("invalid key derivation parameters: {0}")]
    Params(String),
    #[error("key derivation failed: {0}")]
    Derive(String),
}

/// Fresh random salt for a new person on this device.
pub fn new_salt() -> [u8; SALT_LEN] {
    *uuid::Uuid::new_v4().as_bytes()
}

/// Derives the 32-byte database key from `password` and `salt`.
pub fn derive_local_key(
    password: &[u8],
    salt: &[u8; SALT_LEN],
) -> Result<[u8; KEY_LEN], LocalKeyError> {
    let params = Params::new(MEMORY_KIB, PASSES, LANES, Some(KEY_LEN))
        .map_err(|e| LocalKeyError::Params(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| LocalKeyError::Derive(e.to_string()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_password_and_salt_give_the_same_key() {
        let salt = [7u8; SALT_LEN];
        let a = derive_local_key(b"correct horse", &salt).unwrap();
        let b = derive_local_key(b"correct horse", &salt).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_LEN);
    }

    #[test]
    fn different_password_or_salt_gives_a_different_key() {
        let salt = [7u8; SALT_LEN];
        let base = derive_local_key(b"correct horse", &salt).unwrap();
        assert_ne!(base, derive_local_key(b"wrong horse", &salt).unwrap());
        assert_ne!(
            base,
            derive_local_key(b"correct horse", &[8u8; SALT_LEN]).unwrap()
        );
    }

    #[test]
    fn new_salts_are_unique() {
        assert_ne!(new_salt(), new_salt());
    }
}

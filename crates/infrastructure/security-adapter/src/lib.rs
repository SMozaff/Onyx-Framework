//! Production security adapters for Increment 7.
//!
//! `password` and `user_store` were added by the production-readiness audit
//! (finding H-01) to replace api-server's compile-time credential constants.

pub mod authority;
pub mod password;
pub mod rate_limiter;
pub mod secret_provider;
pub mod user_store;

pub use authority::{Ed25519AuthorityVerifier, Ed25519JwtCodec};
pub use password::{constant_time_eq, PasswordError, PasswordHasher};
pub use rate_limiter::{InMemorySlidingWindowRateLimiter, PostgresSlidingWindowRateLimiter};
pub use secret_provider::EnvironmentSecretProvider;
pub use user_store::{PostgresUserStore, SqliteUserStore};

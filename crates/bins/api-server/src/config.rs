//! Validated process configuration for the ONYX API composition root.
//!
//! The API must never infer a security posture from an arbitrary string.
//! `ONYX_ENV` is parsed once into a closed set of supported environments,
//! then all production-sensitive invariants are validated before application
//! state is assembled.

use std::{env, net::SocketAddr};

use axum::http::HeaderValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Test,
    Production,
}

impl Environment {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim() {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            other => anyhow::bail!(
                "invalid ONYX_ENV={other:?}; expected one of: development, test, production"
            ),
        }
    }

    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }

    pub fn allows_development_seed(self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    environment: Environment,
    database_url: String,
    governance_database_url: Option<String>,
    cors_allowed_origins: Option<Vec<HeaderValue>>,
    bind: String,
    metrics_bind: SocketAddr,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://onyx-team7.db?mode=rwc".to_string());
        Self::load(database_url)
    }

    /// Build configuration from process environment while replacing only the
    /// database URL. This preserves the existing `ApiState::new(database_url)`
    /// API used by integration tests and explicit test harnesses.
    pub fn for_database(database_url: &str) -> anyhow::Result<Self> {
        Self::load(database_url.to_owned())
    }

    fn load(database_url: String) -> anyhow::Result<Self> {
        let environment_raw = env::var("ONYX_ENV").unwrap_or_else(|_| "development".to_string());
        let environment = Environment::parse(&environment_raw)?;

        let governance_database_url = env::var("ONYX_GOVERNANCE_DATABASE_URL").ok();

        let cors_allowed_origins = env::var("ONYX_CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|raw| Self::parse_cors_origins(&raw))
            .transpose()?
            .filter(|origins| !origins.is_empty());

        let bind = env::var("ONYX_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
        let metrics_bind: SocketAddr = env::var("ONYX_METRICS_BIND")
            .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid ONYX_METRICS_BIND: {error}"))?;

        let signing_key_present = env::var("ONYX_AUTHORITY_SIGNING_KEY").is_ok();
        Self::validate_invariants(
            environment,
            &database_url,
            governance_database_url.is_some(),
            signing_key_present,
            cors_allowed_origins.is_some(),
        )?;

        Ok(Self {
            environment,
            database_url,
            governance_database_url,
            cors_allowed_origins,
            bind,
            metrics_bind,
        })
    }

    fn parse_cors_origins(raw: &str) -> anyhow::Result<Vec<HeaderValue>> {
        raw.split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(HeaderValue::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                anyhow::anyhow!("ONYX_CORS_ALLOWED_ORIGINS contains an invalid origin: {error}")
            })
    }

    fn validate_invariants(
        environment: Environment,
        database_url: &str,
        governance_database_configured: bool,
        signing_key_configured: bool,
        cors_configured: bool,
    ) -> anyhow::Result<()> {
        if !environment.is_production() {
            return Ok(());
        }

        let postgres_primary =
            database_url.starts_with("postgres://") || database_url.starts_with("postgresql://");
        if !postgres_primary {
            anyhow::bail!(
                "production API storage must use PostgreSQL; per-instance SQLite cannot back a scaled deployment"
            );
        }
        if !signing_key_configured {
            anyhow::bail!("ONYX_AUTHORITY_SIGNING_KEY is required in production");
        }
        if !governance_database_configured {
            anyhow::bail!("ONYX_GOVERNANCE_DATABASE_URL is required in production");
        }
        if !cors_configured {
            anyhow::bail!(
                "ONYX_CORS_ALLOWED_ORIGINS is required in production and must contain at least one origin"
            );
        }

        Ok(())
    }

    pub fn environment(&self) -> Environment {
        self.environment
    }

    pub fn is_production(&self) -> bool {
        self.environment.is_production()
    }

    pub fn development_seed_enabled(&self) -> bool {
        self.environment.allows_development_seed()
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn database_is_postgres(&self) -> bool {
        self.database_url.starts_with("postgres://")
            || self.database_url.starts_with("postgresql://")
    }

    pub fn governance_database_url(&self) -> Option<&str> {
        self.governance_database_url.as_deref()
    }

    pub fn cors_allowed_origins(&self) -> Option<&[HeaderValue]> {
        self.cors_allowed_origins.as_deref()
    }

    pub fn bind(&self) -> &str {
        &self.bind
    }

    pub fn metrics_bind(&self) -> SocketAddr {
        self.metrics_bind
    }

    pub fn storage_backend(&self) -> &'static str {
        if self.database_is_postgres() {
            "postgres"
        } else {
            "sqlite"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_environments_are_closed_and_exact() {
        assert_eq!(
            Environment::parse("development").unwrap(),
            Environment::Development
        );
        assert_eq!(Environment::parse("test").unwrap(), Environment::Test);
        assert_eq!(
            Environment::parse("production").unwrap(),
            Environment::Production
        );

        assert!(Environment::parse("prod").is_err());
        assert!(Environment::parse("staging").is_err());
        assert!(Environment::parse("PRODUCTION").is_err());
        assert!(Environment::parse("").is_err());
    }

    #[test]
    fn production_invariants_require_all_security_prerequisites() {
        let missing_everything = AppConfig::validate_invariants(
            Environment::Production,
            "sqlite::memory:",
            false,
            false,
            false,
        );
        assert!(missing_everything.is_err());

        let missing_governance = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            false,
            true,
            true,
        );
        assert!(missing_governance.is_err());

        let missing_signing_key = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            false,
            true,
        );
        assert!(missing_signing_key.is_err());

        let missing_cors = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            true,
            false,
        );
        assert!(missing_cors.is_err());

        assert!(AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            true,
            true,
        )
        .is_ok());
    }

    #[test]
    fn non_production_environments_keep_local_compositions_possible() {
        assert!(AppConfig::validate_invariants(
            Environment::Development,
            "sqlite::memory:",
            false,
            false,
            false,
        )
        .is_ok());
        assert!(AppConfig::validate_invariants(
            Environment::Test,
            "sqlite::memory:",
            false,
            false,
            false,
        )
        .is_ok());
    }
}

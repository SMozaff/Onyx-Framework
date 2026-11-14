//! Admin sign-in (Phase 1).
//!
//! The request is sent from Rust, not from the UI, with `client_type: "admin"`.
//! Tokens are kept in memory only (decision Q-01): nothing here touches disk,
//! the OS keyring, or browser storage.
//!
//! The All-Father account has the reserved id `__allfather__`, which is not a
//! UUID. The id is accepted as an opaque string on purpose.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Canonical ONYX API base. Must match the Tauri admin-shell and the web UI.
const API_BASE: &str = match option_env!("ONYX_API_ORIGIN") {
    Some(origin) => origin,
    None => "https://onyx-framework.soheil-mozaffari.workers.dev",
};
const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
const KNOWN_ROLES: [&str; 3] = ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"];

#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    client_type: &'static str,
}

#[derive(Deserialize)]
struct LoginWire {
    access_token: String,
    refresh_token: String,
    user: UserWire,
}

#[derive(Deserialize)]
struct UserWire {
    id: String,
    username: String,
    organization_id: String,
    is_admin: bool,
    class: Option<String>,
    role: String,
}

/// A signed-in Admin session. Holds tokens in memory for the life of the
/// process. Deliberately has no `Debug` or `Clone`, so tokens cannot end up
/// in logs or copies.
#[allow(dead_code)] // fields are read by later Phase 1 steps
pub struct AdminSession {
    pub user_id: String,
    pub username: String,
    pub organization_id: String,
    pub role: String,
    pub is_admin: bool,
    access_token: String,
    refresh_token: String,
}

/// Failure kinds for the UI. Never carries the response body or tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginError {
    InvalidCredentials,
    Timeout,
    Network,
    Server { status: u16 },
    InvalidResponse,
}

/// Parses a successful login body into a session. Pure, so it can be tested
/// without a network.
pub fn session_from_body(body: &str) -> Result<AdminSession, LoginError> {
    let wire: LoginWire = serde_json::from_str(body).map_err(|_| LoginError::InvalidResponse)?;
    if wire.access_token.is_empty() || wire.refresh_token.is_empty() {
        return Err(LoginError::InvalidResponse);
    }
    let user = wire.user;
    if user.id.is_empty()
        || user.username.is_empty()
        || user.organization_id.is_empty()
        || !KNOWN_ROLES.contains(&user.role.as_str())
    {
        return Err(LoginError::InvalidResponse);
    }
    let _ = user.class; // accepted as null or string; not shown in Phase 1
    Ok(AdminSession {
        user_id: user.id,
        username: user.username,
        organization_id: user.organization_id,
        role: user.role,
        is_admin: user.is_admin,
        access_token: wire.access_token,
        refresh_token: wire.refresh_token,
    })
}

/// Sends `POST /api/auth/login` as the Admin client.
pub async fn sign_in(
    username: &str,
    password: &str,
    request_id: &str,
) -> Result<AdminSession, LoginError> {
    let client = reqwest::Client::builder()
        .timeout(LOGIN_TIMEOUT)
        .build()
        .map_err(|_| LoginError::Network)?;
    let response = client
        .post(format!("{API_BASE}/api/auth/login"))
        .header("X-ONYX-Request-ID", request_id)
        .json(&LoginRequest {
            username,
            password,
            client_type: "admin",
        })
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                LoginError::Timeout
            } else {
                LoginError::Network
            }
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(LoginError::InvalidCredentials);
    }
    if !status.is_success() {
        return Err(LoginError::Server {
            status: status.as_u16(),
        });
    }
    let body = response
        .text()
        .await
        .map_err(|_| LoginError::InvalidResponse)?;
    session_from_body(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLFATHER_BODY: &str = r#"{
        "access_token": "test-access",
        "refresh_token": "test-refresh",
        "expires_in": 900,
        "user": {
            "id": "__allfather__",
            "username": "fanoos",
            "organization_id": "__allfather__",
            "is_admin": true,
            "class": null,
            "role": "ALL_FATHER"
        }
    }"#;

    #[test]
    fn accepts_allfather_reserved_id_without_uuid_parsing() {
        let session = session_from_body(ALLFATHER_BODY).expect("valid session");
        assert_eq!(session.user_id, "__allfather__");
        assert_eq!(session.username, "fanoos");
        assert_eq!(session.role, "ALL_FATHER");
        assert!(session.is_admin);
    }

    #[test]
    fn rejects_unknown_role() {
        let body = ALLFATHER_BODY.replace("ALL_FATHER", "GUEST");
        assert_eq!(
            session_from_body(&body).err(),
            Some(LoginError::InvalidResponse)
        );
    }

    #[test]
    fn rejects_empty_tokens() {
        let body = ALLFATHER_BODY.replace("test-refresh", "");
        assert_eq!(
            session_from_body(&body).err(),
            Some(LoginError::InvalidResponse)
        );
    }

    #[test]
    fn rejects_malformed_body() {
        assert_eq!(
            session_from_body("not json").err(),
            Some(LoginError::InvalidResponse)
        );
    }

    #[test]
    fn request_declares_admin_client() {
        let body = serde_json::to_value(LoginRequest {
            username: "fanoos",
            password: "x",
            client_type: "admin",
        })
        .expect("serialize");
        assert_eq!(body["client_type"], "admin");
    }

    /// Guard for decision Q-01: the Admin sign-in code must not write tokens
    /// to disk, the OS keyring, or browser storage.
    #[test]
    fn token_code_has_no_persistent_storage() {
        let sources = [include_str!("auth.rs"), include_str!("main.rs")];
        let forbidden = [
            concat!("std::", "fs"),
            concat!("keyr", "ing"),
            concat!("session", "Storage"),
            concat!("local", "Storage"),
            concat!("File", "::create"),
        ];
        for source in sources {
            // Code only: comments may name these things.
            let code: String = source
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for pattern in forbidden {
                assert!(!code.contains(pattern), "forbidden storage use: {pattern}");
            }
        }
    }
}

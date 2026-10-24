//! Native admin sign-in.
//!
//! The Admin Shell's login request is sent from Rust (reqwest), not from the
//! WebView. This avoids depending on the platform's WebView network stack
//! (WebView2 on Windows, WebKitGTK on Linux) for the one request that must
//! succeed before anything else works. The same pattern is used by
//! `desktop-shell`'s `session::authenticate`.
//!
//! The endpoint is the immutable canonical ONYX Cloud API address, the same
//! value the UI uses. Callers cannot supply a different endpoint.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Canonical ONYX Cloud API base. Must match `serverAddress.ts` in the UI.
const CANONICAL_API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

/// Matches the UI's previous 30 s login timeout.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Input from the UI. Deliberately has no `Debug` derive, so the password
/// can't end up in logs.
#[derive(Deserialize)]
pub struct AdminLoginInput {
    pub username: String,
    pub password: String,
    pub request_id: String,
}

#[derive(Serialize)]
struct WireLoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    client_type: &'static str,
}

/// Failure categories sent back to the UI. Carries only the HTTP status and
/// the server's machine code. Never the response body, password, or tokens.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdminLoginError {
    Http { status: u16, code: Option<String> },
    Timeout,
    Connect,
    Network,
    InvalidResponse,
}

/// Sends `POST /api/auth/login` and returns the server's JSON body on success
/// (the token pair and user, validated by the UI as before).
#[tauri::command]
pub async fn admin_login(input: AdminLoginInput) -> Result<serde_json::Value, AdminLoginError> {
    let client = reqwest::Client::builder()
        .timeout(LOGIN_TIMEOUT)
        .build()
        .map_err(|_| AdminLoginError::Network)?;

    let response = client
        .post(format!("{CANONICAL_API_BASE}/api/auth/login"))
        .header("X-ONYX-Request-ID", &input.request_id)
        .json(&WireLoginRequest {
            username: &input.username,
            password: &input.password,
            client_type: "admin",
        })
        .send()
        .await
        .map_err(classify_send_error)?;

    let status = response.status();
    if !status.is_success() {
        // The Worker's error envelope is {"error", "code", ...}. Only `code`
        // is read, and it is returned as a machine code, not as text.
        let code = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|value| {
                value
                    .get("code")
                    .and_then(|c| c.as_str())
                    .map(str::to_owned)
            });
        return Err(AdminLoginError::Http {
            status: status.as_u16(),
            code,
        });
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|_| AdminLoginError::InvalidResponse)
}

fn classify_send_error(error: reqwest::Error) -> AdminLoginError {
    if error.is_timeout() {
        AdminLoginError::Timeout
    } else if error.is_connect() {
        AdminLoginError::Connect
    } else {
        AdminLoginError::Network
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_error_serializes_as_machine_code_only() {
        let value = serde_json::to_value(AdminLoginError::Http {
            status: 401,
            code: Some("INVALID_CREDENTIALS".to_owned()),
        })
        .expect("serialize");
        assert_eq!(value["kind"], "http");
        assert_eq!(value["status"], 401);
        assert_eq!(value["code"], "INVALID_CREDENTIALS");
    }

    #[test]
    fn transport_errors_carry_no_extra_fields() {
        let value = serde_json::to_value(AdminLoginError::Timeout).expect("serialize");
        assert_eq!(value, serde_json::json!({ "kind": "timeout" }));
    }
}

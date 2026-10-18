//! HTTP integration regressions for live principal revalidation.
//!
//! A correctly signed access token must stop working when its account is
//! disabled or when the authoritative tenant binding changes after issuance.

use std::net::SocketAddr;

use api_server::routes::ApiState;
use reqwest::{Client, StatusCode};
use security_application::NewUser;
use sqlx::SqlitePool;

const ORG_A: &str = "11111111-1111-1111-1111-111111111111";
const ORG_B: &str = "33333333-3333-3333-3333-333333333333";
const USERNAME: &str = "live-principal-test";
const PASSWORD: &str = "live-principal-test-password";

struct Fixture {
    state: ApiState,
    pool: SqlitePool,
    base: String,
    http: Client,
    user_id: String,
    database_url: String,
}

async fn start(label: &str) -> Fixture {
    let db_path = std::env::temp_dir().join(format!(
        "onyx-live-principal-{label}-{}.db",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let state = ApiState::new(&database_url).await.expect("API state");
    let user = state
        .user_store
        .create(NewUser {
            user_id: uuid::Uuid::new_v4().to_string(),
            username: USERNAME.to_owned(),
            organization_id: ORG_A.to_owned(),
            password_hash: state
                .password_hasher
                .hash(PASSWORD)
                .expect("password hash"),
            is_admin: true,
            is_manager: false,
            class: None,
            parent_user_id: None,
        })
        .await
        .expect("create test user");

    let app = api_server::routes::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let addr: SocketAddr = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let pool = SqlitePool::connect(&database_url)
        .await
        .expect("test database connection");
    Fixture {
        state,
        pool,
        base: format!("http://{addr}"),
        http: Client::new(),
        user_id: user.user_id,
        database_url,
    }
}

async fn login(fixture: &Fixture) -> String {
    let response = fixture
        .http
        .post(format!("{}/api/auth/login", fixture.base))
        .json(&serde_json::json!({
            "username": USERNAME,
            "password": PASSWORD,
            "client_type": "web"
        }))
        .send()
        .await
        .expect("login request");
    assert_eq!(response.status(), StatusCode::OK);
    response.json::<serde_json::Value>().await.expect("login JSON")
        ["access_token"]
        .as_str()
        .expect("access token")
        .to_owned()
}

#[tokio::test]
async fn signed_access_token_is_rejected_after_account_deactivation() {
    let fixture = start("deactivated").await;
    let token = login(&fixture).await;

    fixture
        .state
        .user_store
        .set_active(&fixture.user_id, ORG_A, false)
        .await
        .expect("deactivate user");

    let response = fixture
        .http
        .get(format!("{}/api/users/hierarchy", fixture.base))
        .bearer_auth(token)
        .send()
        .await
        .expect("protected request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signed_access_token_is_rejected_after_tenant_binding_changes() {
    let fixture = start("tenant-change").await;
    let token = login(&fixture).await;

    sqlx::query("UPDATE users SET organization_id = ?1 WHERE id = ?2")
        .bind(ORG_B)
        .bind(&fixture.user_id)
        .execute(&fixture.pool)
        .await
        .expect("move user to a different authoritative tenant");

    let response = fixture
        .http
        .get(format!("{}/api/users/hierarchy", fixture.base))
        .bearer_auth(token)
        .send()
        .await
        .expect("protected request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

//! Two-organization HTTP regression coverage for the administrative identity
//! surface. These tests deliberately seed the second tenant through the store:
//! production HTTP requests must never be able to choose that tenant.

use std::net::SocketAddr;

use reqwest::{Client, StatusCode};
use security_application::NewUser;

const ORG_A: &str = "11111111-1111-1111-1111-111111111111";
const ORG_B: &str = "33333333-3333-3333-3333-333333333333";
const PASSWORD: &str = "tenant-isolation-password";

struct TenantFixture {
    base: String,
    http: Client,
    tenant_a_token: String,
    tenant_b_token: String,
    local_user_id: String,
    foreign_user_id: String,
    foreign_admin_id: String,
}

async fn add_user(
    state: &api_server::routes::ApiState,
    username: &str,
    organization_id: &str,
    is_admin: bool,
) -> String {
    state
        .user_store
        .create(NewUser {
            user_id: uuid::Uuid::new_v4().to_string(),
            username: username.to_owned(),
            organization_id: organization_id.to_owned(),
            password_hash: state
                .password_hasher
                .hash(PASSWORD)
                .expect("test user password hash"),
            is_admin,
            is_manager: false,
            class: None,
            parent_user_id: None,
        })
        .await
        .expect("seed tenant test user")
        .user_id
}

async fn login(http: &Client, base: &str, username: &str) -> String {
    let response = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": username, "password": PASSWORD}))
        .send()
        .await
        .expect("tenant admin login");
    assert_eq!(response.status(), StatusCode::OK);
    response
        .json::<serde_json::Value>()
        .await
        .expect("tenant admin login JSON")["access_token"]
        .as_str()
        .expect("tenant admin access token")
        .to_owned()
}

async fn start_server(label: &str) -> TenantFixture {
    let db_path = std::env::temp_dir().join(format!("onyx-admin-tenant-test-{label}.db"));
    let _ = std::fs::remove_file(&db_path);
    let state =
        api_server::routes::ApiState::new(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await
            .expect("api state");

    let _local_admin_id = add_user(&state, "tenant-a-admin", ORG_A, true).await;
    let foreign_admin_id = add_user(&state, "tenant-b-admin", ORG_B, true).await;
    let local_user_id = add_user(&state, "tenant-a-user", ORG_A, false).await;
    let foreign_user_id = add_user(&state, "tenant-b-user", ORG_B, false).await;

    let app = api_server::routes::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let addr: SocketAddr = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let http = Client::new();
    let base = format!("http://{addr}");
    let tenant_a_token = login(&http, &base, "tenant-a-admin").await;
    let tenant_b_token = login(&http, &base, "tenant-b-admin").await;
    TenantFixture {
        base,
        http,
        tenant_a_token,
        tenant_b_token,
        local_user_id,
        foreign_user_id,
        foreign_admin_id,
    }
}

#[tokio::test]
async fn tenant_admin_cannot_enumerate_or_read_foreign_accounts() {
    let fixture = start_server("enumerate-read").await;

    let listed = fixture
        .http
        .get(format!("{}/api/admin/users", fixture.base))
        .bearer_auth(&fixture.tenant_a_token)
        .send()
        .await
        .expect("tenant A list request");
    assert_eq!(listed.status(), StatusCode::OK);
    let users: Vec<serde_json::Value> = listed.json().await.expect("tenant A list JSON");
    assert!(users.iter().all(|user| user["organization_id"] == ORG_A));
    assert!(users.iter().any(|user| user["id"] == fixture.local_user_id));
    assert!(users
        .iter()
        .all(|user| user["id"] != fixture.foreign_user_id));
    assert!(users
        .iter()
        .all(|user| user["id"] != fixture.foreign_admin_id));

    let foreign_read = fixture
        .http
        .get(format!(
            "{}/api/admin/users/{}",
            fixture.base, fixture.foreign_user_id
        ))
        .bearer_auth(&fixture.tenant_a_token)
        .send()
        .await
        .expect("foreign read request");
    assert_eq!(foreign_read.status(), StatusCode::NOT_FOUND);

    let own_read = fixture
        .http
        .get(format!(
            "{}/api/admin/users/{}",
            fixture.base, fixture.local_user_id
        ))
        .bearer_auth(&fixture.tenant_a_token)
        .send()
        .await
        .expect("local read request");
    assert_eq!(own_read.status(), StatusCode::OK);
    assert_eq!(
        own_read
            .json::<serde_json::Value>()
            .await
            .expect("local read JSON")["organization_id"],
        ORG_A
    );
}

#[tokio::test]
async fn tenant_admin_cannot_create_or_mutate_foreign_accounts() {
    let fixture = start_server("create-mutate").await;

    // `organization_id` is intentionally not part of CreateUserRequest. Serde
    // ignores this legacy/hostile extra field, and the handler takes scope only
    // from the authenticated administrator's stored organization.
    let created = fixture
        .http
        .post(format!("{}/api/admin/users", fixture.base))
        .bearer_auth(&fixture.tenant_a_token)
        .json(&serde_json::json!({
            "username": "attempted-cross-tenant-create",
            "password": PASSWORD,
            "organization_id": ORG_B,
        }))
        .send()
        .await
        .expect("tenant A create request");
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(
        created
            .json::<serde_json::Value>()
            .await
            .expect("create JSON")["organization_id"],
        ORG_A,
        "caller-provided organization_id must never select the new user's tenant"
    );

    let foreign_path = format!(
        "{}/api/admin/users/{}",
        fixture.base, fixture.foreign_user_id
    );
    let responses = [
        fixture
            .http
            .post(format!("{foreign_path}/deactivate"))
            .bearer_auth(&fixture.tenant_a_token)
            .send()
            .await
            .expect("foreign deactivate"),
        fixture
            .http
            .post(format!("{foreign_path}/activate"))
            .bearer_auth(&fixture.tenant_a_token)
            .send()
            .await
            .expect("foreign activate"),
        fixture
            .http
            .post(format!("{foreign_path}/password"))
            .bearer_auth(&fixture.tenant_a_token)
            .json(&serde_json::json!({"password": "replacement-password"}))
            .send()
            .await
            .expect("foreign password reset"),
        fixture
            .http
            .post(format!("{foreign_path}/manager"))
            .bearer_auth(&fixture.tenant_a_token)
            .json(&serde_json::json!({"is_manager": true}))
            .send()
            .await
            .expect("foreign manager change"),
        fixture
            .http
            .post(format!("{foreign_path}/class"))
            .bearer_auth(&fixture.tenant_a_token)
            .json(&serde_json::json!({"class": "team_leader"}))
            .send()
            .await
            .expect("foreign class change"),
        fixture
            .http
            .post(format!("{foreign_path}/parent"))
            .bearer_auth(&fixture.tenant_a_token)
            .json(&serde_json::json!({"parent_user_id": fixture.local_user_id}))
            .send()
            .await
            .expect("foreign parent change"),
    ];
    for response in responses {
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    let foreign_after = fixture
        .http
        .get(&foreign_path)
        .bearer_auth(&fixture.tenant_b_token)
        .send()
        .await
        .expect("foreign admin read");
    assert_eq!(foreign_after.status(), StatusCode::OK);
    let foreign_after: serde_json::Value = foreign_after.json().await.expect("foreign read JSON");
    assert_eq!(foreign_after["organization_id"], ORG_B);
    assert_eq!(foreign_after["is_active"], true);
    assert_eq!(foreign_after["is_manager"], false);
    assert_eq!(foreign_after["class"], serde_json::Value::Null);
    assert_eq!(foreign_after["parent_user_id"], serde_json::Value::Null);
}

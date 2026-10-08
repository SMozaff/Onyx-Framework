//! Push delivery worker (PWA ObserverClient notification fan-out).
//!
//! Every [`poll_interval`](PushDeliveryConfig::poll_interval) the worker:
//!   1. loads every `push_subscriptions` row (endpoints registered by the
//!      api-server push routes),
//!   2. finds that owner's `notification` aggregates that are
//!      `status = 'unacknowledged'` and not yet delivered to the subscription
//!      (ledger: `push_deliveries`, migration
//!      `20260112000000_add_push_deliveries`),
//!   3. VAPID-signs a JWT and encrypts the payload (RFC 8292 / RFC 8291 via
//!      [`crate::webpush`]),
//!   4. POSTs the message; on success records a delivery row (idempotent per
//!      `(subscription_id, notification_id)`), on `404/410` prunes the stale
//!      subscription, and on any transient failure leaves the notification
//!      pending for the next tick — the ledger is only ever updated on
//!      success.
//!
//! Targeting: a notification's `state.recipient_id` must equal the
//! subscription's `user_id` (the session id that registered the push
//! subscription). The notification aggregate itself stays authoritative for
//! the UI's unread badge in api-server's `query_handler`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};
use ring::agreement;
use serde_json::{json, Value};
use sqlx::postgres::PgPool;

use crate::webpush::{self, VapidSigner};

/// Tuning knobs for the delivery loop. All fields have sane defaults; the
/// binary's `main` overlays environment overrides via [`Self::from_env`].
#[derive(Debug, Clone)]
pub struct PushDeliveryConfig {
    /// How long to sleep between full circulation passes.
    pub poll_interval: Duration,
    /// `TTL` header: how long the push service may hold the message if the
    /// app is offline.
    pub ttl_seconds: u64,
    /// Max notifications delivered to a single subscription per tick (bounds
    /// the fan-out burst when an app comes back online).
    pub max_notifications_per_subscription: i64,
    /// VAPID `sub` claim (e.g. `mailto:ops@example.com`).
    pub vapid_subject: String,
    /// HTTP client per-request timeout toward the push endpoint.
    pub http_timeout: Duration,
}

impl Default for PushDeliveryConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(10),
            ttl_seconds: 86_400,
            max_notifications_per_subscription: 50,
            vapid_subject: "mailto:operations@onyx.local".to_string(),
            http_timeout: Duration::from_secs(15),
        }
    }
}

impl PushDeliveryConfig {
    /// Applies `ONYX_PUSH_*` environment overrides over the defaults.
    pub fn from_env() -> Self {
        let parse_u64 = |k: &str| std::env::var(k).ok().and_then(|v| v.parse().ok());
        let parse_i64 = |k: &str| std::env::var(k).ok().and_then(|v| v.parse().ok());
        Self {
            poll_interval: parse_u64("ONYX_PUSH_POLL_INTERVAL_SECS")
                .map(Duration::from_secs)
                .unwrap_or_else(|| Duration::from_secs(10)),
            ttl_seconds: parse_u64("ONYX_PUSH_TTL_SECS").unwrap_or(86_400),
            max_notifications_per_subscription: parse_i64("ONYX_PUSH_MAX_BATCH").unwrap_or(50),
            http_timeout: parse_u64("ONYX_PUSH_HTTP_TIMEOUT_SECS")
                .map(Duration::from_secs)
                .unwrap_or_else(|| Duration::from_secs(15)),
            vapid_subject: std::env::var("ONYX_VAPID_SUBJECT")
                .unwrap_or_else(|_| "mailto:operations@onyx.local".to_string()),
        }
    }
}

/// Outcome of a single POST to a push endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// 2xx — the push service accepted the message.
    Delivered,
    /// 404/410 — the endpoint is stale; the subscription should be pruned.
    Gone,
    /// Anything else (5xx, throttling, network errors, …) — keep the
    /// notification pending and retry on the next tick.
    RetryLater,
    /// A defensive filter (e.g. recipient mismatch) decided nothing should be
    /// sent — not a failure, just never delivered.
    Ignored,
}

/// The one seam the delivery logic depends on: an HTTP round trip to a push
/// endpoint. Production uses [`HttpPushSender`]; tests inject a stub to assert
/// headers/body and exercise every branch without a network.
#[async_trait]
pub trait PushSender: Send + Sync {
    async fn send(&self, endpoint: &str, headers: HeaderMap, body: Vec<u8>) -> SendOutcome;
}

/// Reqwest-backed sender. Network errors and non-2xx statuses map to
/// [`RetryLater`](SendOutcome::RetryLater) so nothing is lost on a blip.
pub struct HttpPushSender {
    client: reqwest::Client,
}

impl HttpPushSender {
    pub fn new(timeout: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .context("failed to build push HTTP client")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl PushSender for HttpPushSender {
    async fn send(&self, endpoint: &str, headers: HeaderMap, body: Vec<u8>) -> SendOutcome {
        let response = match self
            .client
            .post(endpoint)
            .headers(headers)
            .body(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(error) => {
                tracing::debug!(
                    endpoint = %redact_endpoint(endpoint),
                    error_class = "push_transport",
                    %error,
                    "push POST failed; will retry next tick"
                );
                return SendOutcome::RetryLater;
            }
        };
        let status = response.status();
        match status.as_u16() {
            200..=299 => SendOutcome::Delivered,
            404 | 410 => SendOutcome::Gone,
            code => {
                tracing::debug!(
                    endpoint = %redact_endpoint(endpoint),
                    status = code,
                    "push endpoint returned a non-success status; will retry next tick"
                );
                SendOutcome::RetryLater
            }
        }
    }
}

/// Endpoint URLs embed a registration token; log only the origin.
fn redact_endpoint(endpoint: &str) -> String {
    match url::Url::parse(endpoint) {
        Ok(url) => format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or("?"),
            url.port().map(|p| format!(":{p}")).unwrap_or_default()
        ),
        Err(_) => "<unparseable>".to_string(),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Subscription {
    pub(crate) id: uuid::Uuid,
    pub(crate) user_id: uuid::Uuid,
    pub(crate) organization_id: uuid::Uuid,
    pub(crate) endpoint: String,
    pub(crate) p256dh: String,
    pub(crate) auth: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Notification {
    pub(crate) id: uuid::Uuid,
    pub(crate) state: Value,
}

/// Per-tick counters, useful for tests and `tracing`-level telemetry.
#[derive(Debug, Default)]
pub struct TickStats {
    pub subscriptions: usize,
    pub delivered: usize,
    pub pruned: usize,
    pub skipped: usize,
    pub retries: usize,
    pub errors: usize,
}

/// One full pass over all subscriptions. Runs inside an explicit
/// transaction-free loop; each singe delivery is committed immediately so a
/// crash between sends cannot re-broadcast an already-consumed batch.
pub async fn deliver_tick(
    pool: &PgPool,
    config: &PushDeliveryConfig,
    signer: &VapidSigner,
    sender: &dyn PushSender,
) -> Result<TickStats> {
    let subscriptions: Vec<Subscription> =
        sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, uuid::Uuid, String, String, String)>(
            "SELECT id, user_id, organization_id, endpoint, p256dh, auth FROM push_subscriptions",
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(
            |(id, user_id, organization_id, endpoint, p256dh, auth)| Subscription {
                id,
                user_id,
                organization_id,
                endpoint,
                p256dh,
                auth,
            },
        )
        .collect();

    let mut stats = TickStats {
        subscriptions: subscriptions.len(),
        ..TickStats::default()
    };

    for subscription in &subscriptions {
        let pending = load_pending_notifications(
            pool,
            subscription,
            config.max_notifications_per_subscription,
        )
        .await?;

        for notification in &pending {
            match deliver_notification(subscription, notification, config, signer, sender).await {
                Ok(SendOutcome::Delivered) => {
                    record_delivery(pool, subscription.id, notification.id).await?;
                    stats.delivered += 1;
                }
                Ok(SendOutcome::Gone) => {
                    prune_subscription(pool, subscription.id).await?;
                    stats.pruned += 1;
                }
                Ok(SendOutcome::Ignored) => stats.skipped += 1,
                Ok(SendOutcome::RetryLater) => stats.retries += 1,
                Err(error) => {
                    tracing::warn!(
                        subscription_id = %subscription.id,
                        notification_id = %notification.id,
                        error_class = "push_deliver",
                        %error,
                        "push delivery attempt failed"
                    );
                    stats.errors += 1;
                }
            }
        }
    }
    Ok(stats)
}

async fn load_pending_notifications(
    pool: &PgPool,
    subscription: &Subscription,
    limit: i64,
) -> Result<Vec<Notification>> {
    let rows: Vec<(uuid::Uuid, Value)> = sqlx::query_as(
        "SELECT a.id, a.state
         FROM aggregates a
         WHERE a.organization_id = $1
           AND a.aggregate_type = 'notification'
           AND a.state->>'status' = 'unacknowledged'
           AND a.state->>'recipient_id' = $2::text
           AND NOT EXISTS (
               SELECT 1 FROM push_deliveries d
               WHERE d.subscription_id = $3 AND d.notification_id = a.id
           )
         ORDER BY a.updated_at ASC
         LIMIT $4",
    )
    .bind(subscription.organization_id)
    .bind(subscription.user_id.to_string())
    .bind(subscription.id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, state)| Notification { id, state })
        .collect())
}

/// Sends one notification to one subscription, returning the endpoint's
/// verdict so the tick can record/prune/retry accordingly.
async fn deliver_notification(
    subscription: &Subscription,
    notification: &Notification,
    config: &PushDeliveryConfig,
    signer: &VapidSigner,
    sender: &dyn PushSender,
) -> Result<SendOutcome> {
    let recipient = notification
        .state
        .get("recipient_id")
        .and_then(Value::as_str)
        .context("notification state has no recipient_id")?;
    if recipient != subscription.user_id.to_string() {
        // A defensive re-check (the SQL already matches by owner); never
        // deliver someone else's notification to a subscription.
        return Ok(SendOutcome::Ignored);
    }

    let payload = json!({
        "title": notification.state.get("title").cloned().unwrap_or_else(|| Value::String("Onyx".into())),
        "message": notification.state.get("message").cloned().unwrap_or(Value::Null),
        "url": "/notifications",
    });

    let rng = ring::rand::SystemRandom::new();
    let (headers, ephemeral) = build_headers(subscription, config, signer, &rng)?;
    let body = webpush::encrypt_payload(
        &serde_json::to_vec(&payload).context("serialize push payload")?,
        &subscription.p256dh,
        &subscription.auth,
        ephemeral,
        &rng,
    )?;
    Ok(sender.send(&subscription.endpoint, headers, body).await)
}

// -- HTTP header construction ------------------------------------------------

static TTL: HeaderName = HeaderName::from_static("ttl");
static CONTENT_ENCODING: HeaderName = HeaderName::from_static("content-encoding");
static URGENCY: HeaderName = HeaderName::from_static("urgency");

fn build_headers(
    subscription: &Subscription,
    config: &PushDeliveryConfig,
    signer: &VapidSigner,
    rng: &ring::rand::SystemRandom,
) -> Result<(HeaderMap, ring::agreement::EphemeralPrivateKey)> {
    let audience = webpush::endpoint_audience(&subscription.endpoint)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock before epoch")?
        .as_secs();
    let token =
        signer.sign_auth_token(&audience, &config.vapid_subject, now, config.ttl_seconds)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!(
            "vapid t={}, k={}",
            token,
            signer.public_key_base64url()
        ))
        .context("build VAPID Authorization header")?,
    );
    headers.insert(
        TTL.clone(),
        HeaderValue::from_str(&config.ttl_seconds.to_string()).context("build TTL header")?,
    );
    headers.insert(
        CONTENT_ENCODING.clone(),
        HeaderValue::from_static("aes128gcm"),
    );
    headers.insert(URGENCY.clone(), HeaderValue::from_static("normal"));
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );

    let ephemeral = ring::agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, rng)
        .map_err(|e| anyhow::anyhow!("push ECDH key generation failed: {e:?}"))?;
    Ok((headers, ephemeral))
}

async fn record_delivery(
    pool: &PgPool,
    subscription_id: uuid::Uuid,
    notification_id: uuid::Uuid,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO push_deliveries (subscription_id, notification_id, delivered_at)
         VALUES ($1, $2, $3)
         ON CONFLICT (subscription_id, notification_id) DO NOTHING",
    )
    .bind(subscription_id)
    .bind(notification_id)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await?;
    Ok(())
}

async fn prune_subscription(pool: &PgPool, subscription_id: uuid::Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM push_deliveries WHERE subscription_id = $1")
        .bind(subscription_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM push_subscriptions WHERE id = $1")
        .bind(subscription_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// -- the background loop ------------------------------------------------------

/// The resident background task. Blocks forever, running
/// [`deliver_tick`] every [`poll_interval`](PushDeliveryConfig::poll_interval).
pub async fn run_push_delivery(
    pool: PgPool,
    config: PushDeliveryConfig,
    signer: Arc<VapidSigner>,
    sender: Arc<dyn PushSender>,
) -> anyhow::Result<()> {
    let interval = config.poll_interval;
    tracing::info!(
        poll_interval_secs = interval.as_secs(),
        ttl_seconds = config.ttl_seconds,
        push_delivery = "started"
    );
    loop {
        match deliver_tick(&pool, &config, &signer, sender.as_ref()).await {
            Ok(stats) => {
                if stats.delivered > 0 || stats.pruned > 0 {
                    tracing::info!(
                        subscriptions = stats.subscriptions,
                        delivered = stats.delivered,
                        pruned = stats.pruned,
                        retries = stats.retries,
                        push_delivery = "tick"
                    );
                }
            }
            Err(error) => {
                tracing::error!(error_class = "push_delivery_tick", %error, "push delivery tick failed");
            }
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webpush;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    type CapturedCall = (String, HeaderMap, Vec<u8>);
    type CapturedCalls = Arc<Mutex<Vec<CapturedCall>>>;

    struct StubSender {
        outcome: SendOutcome,
        calls: Arc<AtomicUsize>,
        capture: Option<CapturedCalls>,
    }

    impl StubSender {
        fn fixed(outcome: SendOutcome) -> Self {
            Self {
                outcome,
                calls: Arc::new(AtomicUsize::new(0)),
                capture: None,
            }
        }
    }

    #[async_trait]
    impl PushSender for StubSender {
        async fn send(&self, endpoint: &str, headers: HeaderMap, body: Vec<u8>) -> SendOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(capture) = &self.capture {
                capture
                    .lock()
                    .await
                    .push((endpoint.to_string(), headers, body));
            }
            self.outcome
        }
    }

    fn make_signer() -> Arc<VapidSigner> {
        let (signer, _der) = VapidSigner::generate().expect("vapid key");
        Arc::new(signer)
    }

    fn make_subscription() -> Subscription {
        Subscription {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            organization_id: uuid::Uuid::new_v4(),
            endpoint: "https://push.example.com/regtoken".to_string(),
            p256dh: String::new(),
            auth: String::new(),
        }
    }

    fn make_ua_keys(
        rng: &ring::rand::SystemRandom,
    ) -> (ring::agreement::EphemeralPrivateKey, Vec<u8>, [u8; 16]) {
        let priv_key = ring::agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, rng)
            .expect("ua key");
        let pub_point = priv_key
            .compute_public_key()
            .expect("ua pub")
            .as_ref()
            .to_vec();
        (priv_key, pub_point, [0x42; 16])
    }

    fn fill_subscription_publics(sub: &mut Subscription, pub_point: &[u8], auth: &[u8]) {
        sub.p256dh = webpush::b64url_encode(pub_point);
        sub.auth = webpush::b64url_encode(auth);
    }

    #[tokio::test]
    async fn deliver_notification_builds_vapid_aes128gcm_request() {
        let signer = make_signer();
        let rng = ring::rand::SystemRandom::new();
        let (ua_private, ua_public, auth) = make_ua_keys(&rng);
        let mut sub = make_subscription();
        fill_subscription_publics(&mut sub, &ua_public, &auth);

        let notif = Notification {
            id: uuid::Uuid::new_v4(),
            state: json!({
                "recipient_id": sub.user_id.to_string(),
                "title": "Mission approved",
                "message": "Ops gave the green light",
            }),
        };
        let stub = StubSender::fixed(SendOutcome::Delivered);
        let outcome =
            deliver_notification(&sub, &notif, &PushDeliveryConfig::default(), &signer, &stub)
                .await
                .expect("deliver");
        assert_eq!(outcome, SendOutcome::Delivered, "should report delivered");

        // Verify the assembled request end-to-end through a capturing sender so
        // the encrypted body can be decrypted back with the subscription keys.
        let calls: CapturedCalls = Arc::new(Mutex::new(Vec::new()));
        let stub = StubSender {
            outcome: SendOutcome::Delivered,
            calls: Arc::new(AtomicUsize::new(0)),
            capture: Some(Arc::clone(&calls)),
        };
        deliver_notification(&sub, &notif, &PushDeliveryConfig::default(), &signer, &stub)
            .await
            .expect("deliver");
        let captured = calls.lock().await;
        let (endpoint, headers, body) = &captured[0];
        assert_eq!(endpoint.as_str(), sub.endpoint.as_str());

        let auth_header = headers
            .get(AUTHORIZATION)
            .expect("authorization")
            .to_str()
            .unwrap();
        assert!(
            auth_header.starts_with("vapid t="),
            "VAPID Authorization prefix"
        );
        assert!(auth_header.contains(", k="), "VAPID public key param");

        assert_eq!(headers.get(&TTL).unwrap().to_str().unwrap(), "86400");
        assert_eq!(
            headers.get(&CONTENT_ENCODING).unwrap().to_str().unwrap(),
            "aes128gcm"
        );
        assert_eq!(headers.get(&URGENCY).unwrap().to_str().unwrap(), "normal");

        let plaintext = webpush::decrypt_push_message(body, ua_private, &sub.auth)
            .expect("decrypt push message");
        let payload: Value = serde_json::from_slice(&plaintext).expect("payload json");
        assert_eq!(payload["title"], "Mission approved");
        assert_eq!(payload["message"], "Ops gave the green light");
        assert_eq!(payload["url"], "/notifications");
    }

    #[tokio::test]
    async fn deliver_notification_skips_on_recipient_mismatch() {
        let signer = make_signer();
        let sub = make_subscription();
        let notif = Notification {
            id: uuid::Uuid::new_v4(),
            state: json!({
                "recipient_id": uuid::Uuid::new_v4().to_string(),
                "title": "B",
                "message": "C",
            }),
        };
        let stub = StubSender::fixed(SendOutcome::Delivered);
        let outcome =
            deliver_notification(&sub, &notif, &PushDeliveryConfig::default(), &signer, &stub)
                .await
                .expect("deliver");
        assert_eq!(outcome, SendOutcome::Ignored);
        assert_eq!(
            stub.calls.load(Ordering::SeqCst),
            0,
            "no HTTP call on mismatch"
        );
    }

    #[tokio::test]
    async fn gone_outcome_is_reported_for_pruning() {
        let signer = make_signer();
        let rng = ring::rand::SystemRandom::new();
        let (_, ua_public, auth) = make_ua_keys(&rng);
        let mut sub = make_subscription();
        fill_subscription_publics(&mut sub, &ua_public, &auth);
        let notif = Notification {
            id: uuid::Uuid::new_v4(),
            state: json!({ "recipient_id": sub.user_id.to_string() }),
        };
        let stub = StubSender::fixed(SendOutcome::Gone);
        let outcome =
            deliver_notification(&sub, &notif, &PushDeliveryConfig::default(), &signer, &stub)
                .await
                .expect("deliver");
        assert_eq!(
            outcome,
            SendOutcome::Gone,
            "Gone must be surfaced for pruning"
        );
    }
}

/// Live-PostgreSQL coverage for the *scheduling* surface of the delivery
/// worker: dedup via the `push_deliveries` ledger and pruning of stale
/// `404/410` subscriptions. Requires `DATABASE_URL` (mirrors
/// `staff_loan_scheduler::postgres_integration_tests`); skipped when unset.
#[cfg(test)]
mod postgres_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    // `deliver_tick` is intentionally a *global* scan of every
    // `push_subscriptions` row, so two Postgres-gated tests running in
    // parallel would each deliver the other test's notification too
    // (visible as a delivered count of 2). ctest runs them on separate
    // threads against the same database; serialize them.
    static DB_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    async fn acquire_db() -> tokio::sync::MutexGuard<'static, ()> {
        DB_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    struct SequenceSender {
        outcome: SendOutcome,
        calls: Arc<AtomicUsize>,
    }

    impl SequenceSender {
        fn new(outcome: SendOutcome) -> Self {
            Self {
                outcome,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl PushSender for SequenceSender {
        async fn send(&self, _endpoint: &str, _headers: HeaderMap, _body: Vec<u8>) -> SendOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcome
        }
    }

    async fn seed_subscription_and_notification(
        pool: &sqlx::PgPool,
    ) -> anyhow::Result<(uuid::Uuid, uuid::Uuid, Subscription)> {
        let organization_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let subscription_id = uuid::Uuid::new_v4();
        let notification_id = uuid::Uuid::new_v4();

        // The endpoint's RFC 8291 user-agent key must be real (not a
        // placeholder): `deliver_notification` encrypts the payload with it
        // before the stub sender sees anything, so a bogus base64url string
        // fails decode and the tick would report an error instead of a
        // delivery. Generate a fresh keypair the same way `mod tests` does.
        let rng = ring::rand::SystemRandom::new();
        let ua_private =
            ring::agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
                .expect("ua key");
        let ua_public = ua_private
            .compute_public_key()
            .expect("ua pub")
            .as_ref()
            .to_vec();
        let auth = [0x42; 16];

        sqlx::query(
            "INSERT INTO push_subscriptions (id, user_id, organization_id, endpoint, p256dh, auth, platform, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(subscription_id)
        .bind(user_id)
        .bind(organization_id)
        .bind("https://push.example.com/regtoken")
        .bind(webpush::b64url_encode(&ua_public))
        .bind(webpush::b64url_encode(&auth))
        .bind("pwa")
        .bind(chrono::Utc::now().timestamp_millis())
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO aggregates (id, aggregate_type, organization_id, version, lifecycle_epoch, authority_epoch, state, updated_at) \
             VALUES ($1, 'notification', $2, 1, 0, 0, $3, NOW())",
        )
        .bind(notification_id)
        .bind(organization_id)
        .bind(json!({
            "public_id": notification_id.to_string(),
            "title": "Hello",
            "message": "World",
            "status": "unacknowledged",
            "recipient_id": user_id.to_string(),
        }))
        .execute(pool)
        .await?;

        let sub = Subscription {
            id: subscription_id,
            user_id,
            organization_id,
            endpoint: "https://push.example.com/regtoken".to_string(),
            p256dh: "x".to_string(),
            auth: "x".to_string(),
        };
        Ok((organization_id, notification_id, sub))
    }

    async fn cleanup(
        pool: &sqlx::PgPool,
        organization_id: uuid::Uuid,
        subscription_id: uuid::Uuid,
    ) {
        sqlx::query("DELETE FROM push_subscriptions WHERE id = $1")
            .bind(subscription_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM aggregates WHERE organization_id = $1")
            .bind(organization_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM push_deliveries WHERE subscription_id = $1")
            .bind(subscription_id)
            .execute(pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn deliver_tick_records_ledger_once_and_skips_duplicates() -> anyhow::Result<()> {
        let _guard = acquire_db().await;
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("skipping: DATABASE_URL is unset");
            return Ok(());
        };
        if !database_url.starts_with("postgres") {
            eprintln!("skipping: DATABASE_URL is not PostgreSQL");
            return Ok(());
        }
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await?;
        let (organization_id, _, sub) = seed_subscription_and_notification(&pool).await?;

        let (signer, _der) = VapidSigner::generate().expect("vapid key");
        let sender = std::sync::Arc::new(SequenceSender::new(SendOutcome::Delivered));
        let sender2 = std::sync::Arc::clone(&sender);
        let config = PushDeliveryConfig::default();

        let stats = deliver_tick(&pool, &config, &signer, sender.as_ref()).await?;
        assert_eq!(
            stats.delivered, 1,
            "first tick delivers the pending notification"
        );
        let calls_after_first = sender.calls.load(Ordering::SeqCst);
        assert_eq!(calls_after_first, 1, "exactly one POST");

        let ledger: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM push_deliveries WHERE subscription_id = $1",
        )
        .bind(sub.id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(ledger, 1, "ledger records exactly one delivery");

        let second = deliver_tick(&pool, &config, &signer, sender2.as_ref()).await?;
        assert_eq!(second.delivered, 0, "second tick must not re-deliver");
        assert_eq!(
            sender.calls.load(Ordering::SeqCst),
            calls_after_first,
            "no additional POST for an already-delivered notification"
        );

        cleanup(&pool, organization_id, sub.id).await;
        Ok(())
    }

    #[tokio::test]
    async fn deliver_tick_prunes_gone_subscriptions() -> anyhow::Result<()> {
        let _guard = acquire_db().await;
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("skipping: DATABASE_URL is unset");
            return Ok(());
        };
        if !database_url.starts_with("postgres") {
            eprintln!("skipping: DATABASE_URL is not PostgreSQL");
            return Ok(());
        }
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await?;
        let (organization_id, _, sub) = seed_subscription_and_notification(&pool).await?;

        let (signer, _der) = VapidSigner::generate().expect("vapid key");
        let sender = SequenceSender::new(SendOutcome::Gone);
        let config = PushDeliveryConfig::default();

        let stats = deliver_tick(&pool, &config, &signer, &sender).await?;
        assert_eq!(stats.pruned, 1, "Gone endpoint prunes the subscription");

        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*)::bigint FROM push_subscriptions WHERE id = $1")
                .bind(sub.id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(remaining, 0, "stale subscription is removed");

        cleanup(&pool, organization_id, sub.id).await;
        Ok(())
    }
}

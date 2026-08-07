use std::sync::{Mutex, OnceLock};

use platform_kernel::Timestamp;
use security_adapter::{Ed25519JwtCodec, EnvironmentSecretProvider};
use security_application::{RotatingSecret, SecretProvider, SecretVersion};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test(flavor = "current_thread")]
async fn previous_secret_works_during_grace_and_expires_afterward() {
    let _guard = env_lock().lock().unwrap();
    let now = Timestamp::now().0 / 1_000_000_000;
    std::env::set_var("TEAM7_ROTATING_KEY", "current");
    std::env::set_var("TEAM7_ROTATING_KEY_PREVIOUS", "previous");
    std::env::set_var(
        "TEAM7_ROTATING_KEY_PREVIOUS_VALID_UNTIL_UNIX",
        (now + 60).to_string(),
    );
    let provider = EnvironmentSecretProvider;
    let secret = provider.get("TEAM7_ROTATING_KEY").await.unwrap();
    assert_eq!(secret.verification_candidates(Timestamp::now()).len(), 2);

    std::env::set_var(
        "TEAM7_ROTATING_KEY_PREVIOUS_VALID_UNTIL_UNIX",
        (now - 1).to_string(),
    );
    let expired = provider.get("TEAM7_ROTATING_KEY").await.unwrap();
    assert_eq!(expired.verification_candidates(Timestamp::now()).len(), 1);

    std::env::remove_var("TEAM7_ROTATING_KEY");
    std::env::remove_var("TEAM7_ROTATING_KEY_PREVIOUS");
    std::env::remove_var("TEAM7_ROTATING_KEY_PREVIOUS_VALID_UNTIL_UNIX");
}

#[tokio::test(flavor = "current_thread")]
async fn previous_secret_requires_an_explicit_grace_expiry() {
    let _guard = env_lock().lock().unwrap();
    std::env::set_var("TEAM7_ROTATING_KEY", "current");
    std::env::set_var("TEAM7_ROTATING_KEY_PREVIOUS", "previous");
    std::env::remove_var("TEAM7_ROTATING_KEY_PREVIOUS_VALID_UNTIL_UNIX");
    let provider = EnvironmentSecretProvider;
    assert!(provider.get("TEAM7_ROTATING_KEY").await.is_err());
    std::env::remove_var("TEAM7_ROTATING_KEY");
    std::env::remove_var("TEAM7_ROTATING_KEY_PREVIOUS");
}

#[test]
fn old_jwt_key_is_accepted_only_during_grace_period() {
    let old = RotatingSecret {
        current: SecretVersion {
            value: vec![1_u8; 32],
            valid_until: None,
        },
        previous: None,
    };
    let old_codec = Ed25519JwtCodec::from_rotating_secret(&old).unwrap();
    let token = old_codec
        .encode(&serde_json::json!({"sub":"user"}))
        .unwrap();

    let rotated = RotatingSecret {
        current: SecretVersion {
            value: vec![2_u8; 32],
            valid_until: None,
        },
        previous: Some(SecretVersion {
            value: vec![1_u8; 32],
            valid_until: Some(Timestamp::from_nanos(200)),
        }),
    };
    let rotated_codec = Ed25519JwtCodec::from_rotating_secret(&rotated).unwrap();
    let during: serde_json::Value = rotated_codec
        .decode_at(&token, Timestamp::from_nanos(199))
        .unwrap();
    assert_eq!(during["sub"], "user");
    assert!(rotated_codec
        .decode_at::<serde_json::Value>(&token, Timestamp::from_nanos(200))
        .is_err());
}

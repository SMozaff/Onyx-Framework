use std::collections::HashMap;

use platform_kernel::{ObjectId, Timestamp};
use security_adapter::InMemorySlidingWindowRateLimiter;
use security_application::{RateLimitPolicy, RateLimitRequest, RateLimiter, ResourceClass};

#[tokio::test]
async fn sliding_window_exceeds_and_resets() {
    let limiter = InMemorySlidingWindowRateLimiter::with_policies(HashMap::from([(
        ResourceClass::Commands,
        RateLimitPolicy {
            limit: 2,
            window_seconds: 10,
        },
    )]));
    let request = |id: &str, seconds: u64| RateLimitRequest {
        organization_id: ObjectId([3; 16]),
        resource_class: ResourceClass::Commands,
        request_id: id.to_string(),
        occurred_at: Timestamp::from_nanos(seconds * 1_000_000_000),
    };
    assert!(
        limiter
            .check_and_record(&request("1", 100))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        limiter
            .check_and_record(&request("2", 101))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        !limiter
            .check_and_record(&request("3", 102))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        limiter
            .check_and_record(&request("4", 111))
            .await
            .unwrap()
            .allowed
    );
}

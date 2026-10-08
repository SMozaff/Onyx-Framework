/// Team 8 ruling R11 keeps client-dependent journeys ignored until Team 5
/// native-client completion and release signing are available.
#[tokio::test]
#[ignore = "Team 5 client event integration is not production-complete"]
async fn journey_5_notification_sync() {
    // Intentionally executable after Team 5 handoff. The backend notification
    // command/query flow is already covered by Team 6 integration tests.
}

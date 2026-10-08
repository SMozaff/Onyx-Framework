mission-operations-platform/
├── Cargo.toml                          # Workspace manifest (22 members)
├── Cargo.lock                          # Pinned dependencies
├── rust-toolchain.toml                 # Rust 1.97.1
├── DECISIONS.md                        # All architectural rulings (central)
├── README.md                           # Quick start + crate overview
├── .cargo/
│   └── config.toml                     # cargo configuration
├── .github/
│   └── workflows/
│       ├── ci.yml                      # Full CI pipeline
│       └── release.yml                 # Release pipeline
├── docs/
│   ├── runbooks/
│   │   ├── incident-response.md
│   │   ├── backup-restore.md
│   │   ├── performance-tuning.md
│   │   ├── on-call.md
│   │   └── disaster-recovery.md
│   └── adr/
│       └── (architecture decision records)
├── scripts/
│   ├── verify/
│   │   ├── verify_contract.sh
│   │   ├── verify_dependencies.sh
│   │   ├── verify_trait_completeness.sh
│   │   ├── verify_error_exhaustiveness.sh
│   │   ├── verify_ffi_signatures.sh
│   │   ├── verify_openapi_spec.sh
│   │   ├── verify_serialization.sh
│   │   ├── verify_crdt_determinism.sh
│   │   ├── verify_tombstone_gc.sh
│   │   ├── verify_authority_controlled.sh
│   │   ├── verify_no_secrets.sh
│   │   ├── verify_structured_logs.sh
│   │   └── verify_migration_idempotency.sh
│   └── ci/
│       ├── run_all_checks.sh
│       └── parse_contracts.py
├── crates/
│   ├── kernel/
│   │   ├── platform-kernel/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── identifiers.rs
│   │   │       ├── versioning.rs
│   │   │       ├── causality.rs
│   │   │       ├── authority.rs
│   │   │       ├── time.rs
│   │   │       └── refs.rs
│   │   └── platform-contracts/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           ├── command.rs
│   │           ├── event.rs
│   │           ├── traits.rs
│   │           └── error.rs
│   ├── domains/
│   │   ├── mission-domain/
│   │   │   ├── Cargo.toml
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── aggregate.rs
│   │   │   │   ├── command.rs
│   │   │   │   ├── event.rs
│   │   │   │   ├── error.rs
│   │   │   │   ├── state_machine.rs
│   │   │   │   └── value.rs
│   │   │   └── tests/
│   │   │       ├── unit.rs
│   │   │       ├── unit/
│   │   │       │   ├── transitions.rs
│   │   │       │   └── invariants.rs
│   │   │       ├── property.rs
│   │   │       └── golden.rs
│   │   └── work-domain/
│   │       ├── Cargo.toml
│   │       ├── src/
│   │       │   ├── lib.rs
│   │       │   ├── aggregate.rs
│   │       │   ├── command.rs
│   │       │   ├── event.rs
│   │       │   ├── error.rs
│   │       │   ├── state_machine.rs
│   │       │   └── value.rs
│   │       └── tests/
│   │           ├── unit.rs
│   │           ├── unit/
│   │           │   ├── transitions.rs
│   │           │   └── invariants.rs
│   │           ├── property.rs
│   │           └── golden.rs
│   ├── applications/
│   │   ├── query-application/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── ports/
│   │   │           ├── mod.rs
│   │   │           ├── repository.rs
│   │   │           ├── unit_of_work.rs
│   │   │           ├── unit_of_work_factory.rs
│   │   │           ├── idempotency_store.rs
│   │   │           └── connection.rs
│   │   ├── worker-application/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── ports/
│   │   │           ├── mod.rs
│   │   │           ├── outbox_store.rs
│   │   │           ├── inbox_store.rs
│   │   │           ├── event_publisher.rs
│   │   │           ├── dead_letter_store.rs
│   │   │           └── job_queue.rs
│   │   ├── security-application/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── ports/
│   │   │           ├── mod.rs
│   │   │           ├── authority_verifier.rs
│   │   │           ├── rate_limiter.rs
│   │   │           └── secret_provider.rs
│   │   ├── audit-application/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── ports/
│   │   │           ├── mod.rs
│   │   │           └── audit_writer.rs
│   │   └── client-composition/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           ├── command_registry.rs
│   │           ├── query_registry.rs
│   │           ├── event_bus.rs
│   │           ├── sync_agent.rs
│   │           ├── app_state.rs
│   │           ├── handlers/
│   │           │   ├── mod.rs
│   │           │   ├── creation_handler.rs
│   │           │   └── decision_handler.rs
│   │           └── test_helpers.rs
│   ├── infrastructure/
│   │   ├── persistence-common/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── timestamp.rs
│   │   │       ├── uuid.rs
│   │   │       └── json.rs
│   │   ├── persistence-postgres/
│   │   │   ├── Cargo.toml
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── repository.rs
│   │   │   │   ├── unit_of_work.rs
│   │   │   │   ├── unit_of_work_factory.rs
│   │   │   │   ├── outbox_store.rs
│   │   │   │   ├── inbox_store.rs
│   │   │   │   ├── dead_letter_store.rs
│   │   │   │   └── idempotency_store.rs
│   │   │   └── tests/
│   │   │       └── integration.rs
│   │   ├── persistence-sqlite/
│   │   │   ├── Cargo.toml
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── repository.rs
│   │   │   │   ├── unit_of_work.rs
│   │   │   │   ├── unit_of_work_factory.rs
│   │   │   │   ├── outbox_store.rs
│   │   │   │   ├── inbox_store.rs
│   │   │   │   ├── dead_letter_store.rs
│   │   │   │   └── idempotency_store.rs
│   │   │   └── tests/
│   │   │       └── integration.rs
│   │   ├── messaging-adapter/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── nats_publisher.rs
│   │   │       └── embedded_publisher.rs
│   │   ├── observability-adapter/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── tracing.rs
│   │   │       ├── metrics.rs
│   │   │       ├── logging.rs
│   │   │       └── audit.rs
│   │   └── security-adapter/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           ├── authority.rs
│   │           ├── rate_limiter.rs
│   │           └── secret_provider.rs
│   ├── sync/
│   │   ├── crdt/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── or_set.rs
│   │   │       ├── lww_register.rs
│   │   │       ├── mv_register.rs
│   │   │       ├── pn_counter.rs
│   │   │       ├── rga.rs
│   │   │       ├── append_only_log.rs
│   │   │       └── tombstone_gc.rs
│   │   ├── synchronization-domain/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── session.rs
│   │   │       ├── merge_strategy.rs
│   │   │       ├── conflict.rs
│   │   │       ├── escalation.rs
│   │   │       └── field_metadata.rs
│   │   └── sync-test-utils/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           └── mocks.rs
│   └── transports/
│       ├── sync-transport/
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs
│       │       ├── transport_trait.rs
│       │       ├── message.rs
│       │       ├── selector.rs
│       │       ├── discovery.rs
│       │       ├── cloud_relay.rs
│       │       ├── wi_fi_direct.rs
│       │       ├── bluetooth_le.rs
│       │       ├── quic_cross_network.rs
│       │       ├── authority_provider.rs
│       │       ├── placeholder_types.rs
│       │       ├── test_support.rs
│       │       └── tests/
│       │           ├── fallback_tests.rs
│       │           ├── encryption_tests.rs
│       │           ├── quic_nat_tests.rs
│       │           ├── discovery_tests.rs
│       │           ├── quota_tests.rs
│       │           └── transport_tests.rs
│       └── sync-transport-mobile/
│           ├── Cargo.toml
│           └── src/
│               ├── lib.rs
│               ├── ios_multipeer.rs
│               ├── ios_ble.rs
│               ├── android_wifi_direct.rs
│               └── android_ble.rs
├── bins/
│   ├── api-server/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── main.rs
│   │   │   ├── command_handler.rs
│   │   │   ├── query_handler.rs
│   │   │   └── middleware/
│   │   │       ├── mod.rs
│   │   │       └── rate_limit.rs
│   │   └── tests/
│   │       └── integration.rs
│   ├── worker/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── outbox_relay.rs
│   │       ├── job_runner.rs
│   │       ├── scheduler_loop.rs
│   │       └── snapshot_loop.rs
│   ├── sync-agent/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── migration-tool/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── migrations.rs
│   ├── desktop-shell/
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── tauri.conf.json
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── main.rs
│   │   │   ├── tauri_commands.rs
│   │   │   ├── tauri_events.rs
│   │   │   ├── secure_storage/
│   │   │   │   ├── mod.rs
│   │   │   │   └── keyring_adapter.rs
│   │   │   └── sync_integration.rs
│   │   └── ui/
│   │       ├── index.html
│   │       ├── src/
│   │       │   ├── App.tsx
│   │       │   ├── main.tsx
│   │       │   ├── hooks/
│   │       │   │   ├── useCommand.ts
│   │       │   │   └── useQuery.ts
│   │       │   └── pages/
│   │       │       ├── Dashboard.tsx
│   │       │       ├── Missions.tsx
│   │       │       ├── Tasks.tsx
│   │       │       └── Approvals.tsx
│   │       └── package.json
│   └── mobile-core/
│       ├── Cargo.toml
│       ├── build.rs
│       ├── src/
│       │   ├── lib.rs
│       │   ├── ffi_commands.rs
│       │   ├── ffi_queries.rs
│       │   ├── ffi_events.rs
│       │   ├── ffi_secure_storage.rs
│       │   ├── ios_background.rs
│       │   ├── android_workmanager.rs
│       │   ├── ios_multipeer.rs
│       │   ├── android_wifi_direct.rs
│       │   ├── ios_ble.rs
│       │   └── android_ble.rs
│       └── tests/
│           └── ffi_integration.rs
├── migrations/
│   ├── 20260101000000_initial_schema.up.sql
│   ├── 20260101000000_initial_schema.down.sql
│   ├── 20260102000000_add_audit_hash.up.sql
│   └── 20260102000000_add_audit_hash.down.sql
├── web-ui/
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── .env.example
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── api/
│       │   ├── client.ts
│       │   ├── command.ts
│       │   ├── query.ts
│       │   └── events.ts
│       ├── hooks/
│       │   ├── useCommand.ts
│       │   ├── useQuery.ts
│       │   ├── useEventStream.ts
│       │   └── useAuth.ts
│       ├── stores/
│       │   ├── authStore.ts
│       │   └── notificationStore.ts
│       ├── types/
│       │   ├── command.ts
│       │   ├── query.ts
│       │   └── events.ts
│       └── pages/
│           ├── Login/
│           ├── Dashboard/
│           ├── Missions/
│           ├── Tasks/
│           ├── Notifications/
│           ├── Approvals/
│           └── Reports/
├── tests/
│   ├── unit/
│   ├── property/
│   ├── integration/
│   ├── contract/
│   ├── component/
│   ├── end-to-end/
│   ├── chaos/
│   └── load/
├── deploy/
│   ├── docker/
│   │   ├── api-server.Dockerfile
│   │   ├── worker.Dockerfile
│   │   ├── sync-agent.Dockerfile
│   │   ├── migration-tool.Dockerfile
│   │   └── desktop-shell.Dockerfile
│   ├── helm/
│   │   ├── onyx-api/
│   │   │   ├── Chart.yaml
│   │   │   ├── values.yaml
│   │   │   └── templates/
│   │   │       ├── deployment.yaml
│   │   │       ├── service.yaml
│   │   │       └── ingress.yaml
│   │   ├── onyx-worker/
│   │   │   ├── Chart.yaml
│   │   │   ├── values.yaml
│   │   │   └── templates/
│   │   │       └── deployment.yaml
│   │   └── onyx-sync-agent/
│   │       ├── Chart.yaml
│   │       ├── values.yaml
│   │       └── templates/
│   │           └── deployment.yaml
│   ├── terraform/
│   │   ├── main.tf
│   │   ├── variables.tf
│   │   └── outputs.tf
│   └── docker-compose.local.yml
├── mobile/
│   ├── pubspec.yaml
│   ├── lib/
│   │   ├── main.dart
│   │   ├── bridge/
│   │   │   └── bridge.dart         # generated by flutter_rust_bridge
│   │   ├── ui/
│   │   │   ├── app.dart
│   │   │   ├── screens/
│   │   │   │   ├── dashboard.dart
│   │   │   │   ├── missions.dart
│   │   │   │   └── tasks.dart
│   │   │   └── widgets/
│   │   │       ├── sync_status.dart
│   │   │       └── conflict_dialog.dart
│   │   └── background/
│   │       ├── ios/
│   │       └── android/
│   ├── android/
│   │   └── app/
│   │       └── src/
│   │           └── main/
│   │               └── kotlin/
│   │                   └── com/onyx/
│   │                       └── WorkManagerService.kt
│   └── ios/
│       └── Runner/
│           ├── AppDelegate.swift
│           └── BackgroundService.swift
└── target/                           # Build artifacts (not in source control)
    ├── debug/
    ├── release/
    └── .sqlx/                        # sqlx prepared query cache

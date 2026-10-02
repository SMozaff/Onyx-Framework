# ONYX Phase 1 — H-01 Environment Configuration Hardening

## Scope

This change addresses the root cause of the H-01 environment-classification defect identified in the security hardening plan.

The defect was not merely the use of a string variable. The deeper problem was that a security posture was inferred throughout the API composition root from a free-form environment string, with every value other than the literal `production` effectively falling into the non-production path.

Examples such as `prod`, `staging`, `PRODUCTION`, or a typo therefore did not fail closed.

## Root-cause fix

The API server now has one authoritative configuration boundary:

```text
process environment
        |
        v
    AppConfig
        |
        +--> Environment
        +--> database requirements
        +--> governance database requirement
        +--> signing-key requirement
        +--> CORS requirement
        +--> bind / metrics configuration
        |
        v
    ApiState
```

`ONYX_ENV` is parsed once into the closed enum:

- `development`
- `test`
- `production`

Any other value is rejected during configuration loading.

The default remains `development` only when `ONYX_ENV` is completely absent, preserving the existing local-development behavior. An explicitly supplied but unknown value is never silently reclassified.

## Production invariants

Production configuration now centrally requires:

1. PostgreSQL for the primary API database.
2. `ONYX_AUTHORITY_SIGNING_KEY`.
3. `ONYX_GOVERNANCE_DATABASE_URL`.
4. A non-empty `ONYX_CORS_ALLOWED_ORIGINS` list.

These checks occur before application state construction.

The development/test seeded administrator is now selected from the typed environment policy through `development_seed_enabled()`, instead of checking whether the raw environment string differs from `production`.

## Composition changes

`main.rs` now loads `AppConfig::from_env()` and passes the validated configuration to `ApiState::new_with_config()`.

The existing `ApiState::new(database_url)` constructor remains as a compatibility boundary for current test harnesses. It uses `AppConfig::for_database()`, so it cannot bypass environment validation.

No application code under `routes/` directly reads `ONYX_ENV` anymore.

## Regression prevention

CI now rejects new direct `ONYX_ENV` reads outside `src/config.rs`.

CI also rejects direct `== "production"` / `!= "production"` posture checks outside `src/config.rs`.

This converts the architectural rule from documentation into an executable repository contract.

## Verification expectations

The Phase 1 acceptance gate is GitHub Actions, not an unchecked local claim.

Required evidence:

- API workspace compilation/tests pass.
- Configuration unit tests pass.
- An unknown environment value is rejected.
- Production configuration rejects missing production prerequisites.
- No direct environment-posture logic remains outside `AppConfig`.
- The Phase 0 `merge-gate` remains green after this change.

## Relationship to Phase 0

This branch is based on the Phase 0 merge-control commit. H-01 is intentionally kept behind the merge-control foundation so the hardening change cannot be merged without the complete CI contract.

## Context7

The implementation follows the typed-configuration boundary used by Rust applications: external configuration is parsed into validated application state before application composition. The GitHub Actions portion continues using independent jobs plus explicit `needs` dependencies, with the aggregate Phase 0 gate remaining the merge-control boundary.

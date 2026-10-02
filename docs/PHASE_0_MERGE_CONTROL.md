# ONYX Phase 0 — Merge-Control Contract

## Objective

Prevent a pull request from being mergeable before the complete CI verification contract has finished.

The implementation has two layers:

1. **Workflow enforcement:** CI exposes one deterministic aggregate `merge-gate` job that fails whenever any required verification dimension fails, is cancelled, or is skipped.
2. **Repository enforcement:** the `main` branch protection/ruleset must require the `merge-gate` status check before merge.

The repository currently reports no GitHub Rulesets. Direct branch-protection inspection is permission-restricted for the available GitHub integration, so the repository-side setting still requires verification by an account with administration access.

## Verification graph

The required verification dimensions are:

- `check`
- `i18n`
- `deploy-check`
- `web`
- `mobile-pwa`
- `native-ui-evidence`
- `load-smoke`
- `mobile-android-kotlin`

`native-ui-evidence` is now independent of `web`. Its previous `needs: web` dependency allowed an unrelated web failure to skip native UI evidence entirely.

`load-smoke` remains dependent on `check` because it deliberately reuses the Rust release-build cache produced by that job. This is a real execution dependency, not merge-governance coupling.

## Aggregate gate semantics

`merge-gate` uses `if: \${{ always() }}` so it executes after its upstream jobs regardless of their outcomes.

It records every upstream result and fails unless all required jobs report `success`.

Therefore:

- failure → merge gate fails
- cancellation → merge gate fails
- skipped → merge gate fails
- all success → merge gate passes

This makes skipped or partially evaluated verification dimensions non-mergeable.

## Repository-side enforcement

For `main`, the repository must enforce:

- pull requests for changes to `main`;
- required status check **`merge-gate`** before merge;
- the repository's chosen up-to-date/base-branch policy;
- no force pushes to `main`;
- no deletion of `main`;
- restricted direct pushes according to the repository's maintainer/automation model.

The workflow change alone does **not** establish these repository rules.

GitHub returned HTTP 403 for the branch-protection endpoint through the available integration, so the current protection configuration has not been independently verified.

## Merge policy

After the repository-side requirement is enabled, `merge-gate` becomes the merge-control contract while the individual jobs remain visible for diagnostics.

Any future release-readiness verification job must be added explicitly to `merge-gate`. Merely defining a new workflow job does not automatically make it part of the merge contract.

## Verification procedure

After this branch is opened as a pull request:

1. Confirm all eight verification jobs and `merge-gate` are created.
2. Confirm `native-ui-evidence` can execute even when `web` fails.
3. Confirm an upstream failure leaves `merge-gate` running and makes it fail.
4. Confirm an upstream skipped result makes `merge-gate` fail.
5. Confirm an all-green run makes `merge-gate` pass.
6. Configure `merge-gate` as the required check for `main`.
7. Use a test pull request to confirm GitHub blocks merge until `merge-gate` is green.

## Context7 basis

The implementation follows GitHub Actions' documented model of parallel jobs plus explicit `needs` dependencies. Independent verification dimensions are kept independent; only genuine execution dependencies remain.

The aggregate gate uses an unconditional job-level execution condition and evaluates the recorded upstream results explicitly, rather than relying on skipped-job behavior.

SQLx and npm hardening are deliberately deferred to later phases; Phase 0 changes only merge-control topology and its repository policy documentation.

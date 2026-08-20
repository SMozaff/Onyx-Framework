# UI/UX Remediation — Milestones 3 and 4 Implementation Record

**Implemented against:** `main` base `c5826e20cd63d0ec0990e3f3b1666194d39a8387`

**Scope:** This increment makes the Milestones 1 and 2 fixes reproducible and enforceable. It adds deterministic web linting, browser accessibility/keyboard/state/visual regression tests, native frontend smoke evidence, CI artifacts, and evidence-retention rules. It does not change tag-triggered publication behavior or silently repair the separate macOS icon asset defect.

## Quality-gate architecture

| Layer | Implementation | Failure prevented | Evidence retained |
|---|---|---|---|
| Static web quality | Tracked `web-ui/.eslintrc.cjs`; `npm run lint -- --max-warnings=0` | Unused state/dead test helpers and common TypeScript/React Hooks defects reaching CI. | CI step result. |
| Type/build quality | Existing TypeScript and production bundle checks retained and made explicit in CI. | Type drift and bundle-budget regression. | CI output and web-quality artifact. |
| Component/integration accessibility | Existing Vitest/axe suite remains independent of browser specs. | Semantic regressions in jsdom-covered components. | JUnit output. |
| Real browser | Playwright Chromium projects, synthetic session/API fixtures, explicit visual snapshot. | Actual CSS/contrast, accessibility, keyboard focus, routing, and unavailable-state regressions. | HTML report, JUnit, failure trace/screenshot, checked-in baseline. |
| Native frontend | Staff/Admin `npm ci && npm run build`, evidence manifest artifact. | Native frontend type/bundle breakage and undocumented check provenance. | `native-ui-frontend-smoke.json`. |
| Target-native runtime | Documented per-platform smoke procedure plus existing manual debug release matrix. | Confusing frontend compilation with a native runtime claim. | Reviewer record and debug matrix URL/artifacts. |

## Changes and decisions

### Deterministic lint restoration

The web package already declared `npm run lint`, but no tracked ESLint configuration existed, so the command could not be relied upon as a quality gate. This increment adds `web-ui/.eslintrc.cjs` with TypeScript and React Hooks recommended rules, an explicit zero-warning requirement, and exclusions for generated outputs and Playwright tests. The restored lint baseline found and removed only two dead declarations: unused draft item state in `TodoTargets/ListDetail.tsx` and an unused generic parameter in the Milestone 1 projection-state test helper.

The installed TypeScript compiler was `5.9.3`, while the prior `@typescript-eslint` v6 toolchain warned that it only supported TypeScript below 5.4. The parser and plugin were therefore upgraded to v8 in the lockfile and package manifest. The enforced lint command now completes without that unsupported-compiler warning.

### Real-browser regression layer

`@playwright/test` and `@axe-core/playwright` now provide a Chromium-only browser layer. The configuration uses a dedicated validation port (`4179`) and prohibits reusing an arbitrary pre-existing development server, so the test always serves the checkout under test. The mobile project intentionally uses an iPhone viewport **with Chromium**; the first local run established that device descriptors otherwise inherit WebKit, which was not installed or selected for this gate.

The synthetic fixture stores only non-sensitive test session values and intercepts only parsed requests with the exact `/api/query` pathname. The exact-path requirement was added after diagnosis showed that a broad glob can intercept Vite’s `src/api/query.ts` module and incorrectly serve JSON as JavaScript. The test suite now verifies the following:

| Browser assertion | Regression prevented |
|---|---|
| Desktop login hero foreground CSS and checked-in screenshot | Return of the audit’s dark-on-dark heading defect. |
| Axe scan of the rendered login hero | Browser-level semantic/contrast regression missed by jsdom. |
| Mobile drawer open, initial focus, Escape close, and trigger focus restoration | Keyboard-inaccessible mobile navigation. |
| Failed Mission projection renders an unavailable recovery panel rather than `0 total` | False-empty operational data. |
| Approval modal naming, focus, optional approval note, Escape close, and Axe scan | Missing modal semantics and Browser/Staff policy drift. |

The visual baseline lives beside the Playwright spec and is generated only with `npm run test:browser:update`; ordinary CI executes `npm run test:browser` and fails on unexpected pixels. Four assertions execute across the two project configurations; four project-inapplicable assertions are intentionally skipped by test code rather than hidden from the report.

### Native smoke evidence and release boundary

`scripts/collect-ui-evidence.mjs` emits a machine-readable manifest after its listed commands succeed. It records commit SHA, platform, architecture, GitHub run identifiers, exact commands, result, and an explicit warning that frontend smoke evidence does not claim a native window launched.

`docs/ui-remediation/NATIVE_UI_SMOKE_EVIDENCE.md` defines three evidence levels: frontend smoke compilation, real GitHub-hosted Tauri debug matrix compilation/bundling, and human-reviewed target-native runtime smoke. It also defines expected recovery, navigation, error, and approval checks for the Staff and Admin shells.

The macOS RGBA icon defect remains an explicit separate asset dependency. The procedure does not present reaching the DMG icon-processing stage as a successful macOS runtime launch. No release tag is created by this work; image and release publication remain controlled solely by the existing `v*` tag policy.

### CI integration and evidence retention

The `web` CI job now runs `npm ci`, lint, type check, existing Vitest suites, Playwright browser regressions, production build/bundle check, and an always-uploaded quality artifact. The new `native-ui-evidence` job depends on web quality success, builds both native frontend packages from locked dependencies, writes the evidence manifest, and uploads it with a 30-day retention period. Web quality reports are retained for 14 days.

The workflow trigger and `release.yml` were not expanded. This avoids publishing or creating release artifacts during ordinary quality validation.

## Local validation before GitHub dispatch

| Command or inspection | Result |
|---|---|
| `web-ui: npm run lint` | Passed with zero warnings. |
| `web-ui: npm run type-check` | Passed. |
| `web-ui: npm test` | Passed: 138 tests passed; 7 existing skips; 1 existing real-server suite skipped. jsdom/axe continues to emit its known canvas warning while assertions pass. |
| `web-ui: npm run test:browser` | Passed: 4 assertions passed; 4 project-inapplicable assertions skipped. |
| `web-ui: npm run build` | Passed with existing bundle check; initial gzip size 112,603 bytes. |
| `Staff UI: npm ci && npm run build` | Passed. |
| `Admin UI: npm ci && npm run build` | Passed. |
| `node scripts/collect-ui-evidence.mjs …` | Passed and produced a schema `1.0` manifest with two recorded commands. |
| `actionlint .github/workflows/ci.yml` and `git diff --check` | Passed. |

## Dependency audit boundary

The package-manager audit output reported existing transitive dependency advisories: web UI installation reported seven findings (four moderate, two high, one critical), while the Staff UI installation reported one high finding; Admin reported none. These reports are recorded because the new CI work surfaced them, but they are not silently remediated here: automatic audit fixes may force unrelated dependency upgrades and are outside the approved UI quality-gate scope. A separately scoped dependency-security update should triage and resolve them with compatibility testing.

## References

- Browser test configuration: `web-ui/playwright.config.ts`
- Browser fixtures and assertions: `web-ui/tests/browser/`
- Native evidence generator: `scripts/collect-ui-evidence.mjs`
- Native smoke procedure: `docs/ui-remediation/NATIVE_UI_SMOKE_EVIDENCE.md`
- CI quality and evidence jobs: `.github/workflows/ci.yml`


## GitHub-hosted validation evidence

The quality implementation was pushed first as `4e062c1`. Its GitHub Actions CI run `32332728269` reached the new web job and uploaded artifact `onyx-web-quality-4e062c1d21c3b227e659e02a85a87187dc44065e` (artifact ID `9393590353`). The new lint, type, existing test, browser keyboard/state/accessibility checks, build, and report upload all completed. The only new-web failure was the original full-text screenshot baseline: GitHub’s rendered heading wrapped differently from the sandbox image, yielding a 10% pixel difference while the foreground contrast and axe assertion passed. The checked-in test was corrected to snapshot the hero background/layout while preserving live text colour and axe assertions separately.

The stable-baseline correction was pushed as `bee8798`. GitHub Actions CI run `32333049774` then completed the new **web** job successfully and completed **native-ui-evidence** successfully. The web quality artifact is `onyx-web-quality-bee8798cfc73718788e3d239935588f39dc81edb` (artifact ID `9393694749`). The native evidence artifact is `onyx-native-ui-evidence-bee8798cfc73718788e3d239935588f39dc81edb` (artifact ID `9393702754`). The downloaded manifest reports schema `1.0`, commit `bee8798cfc73718788e3d239935588f39dc81edb`, Linux x64 runner, GitHub run ID `32333049774`, and both exact Staff/Admin `npm ci && npm run build` commands with `status: passed`.

The overall GitHub run remains red because the pre-existing Rust **check** job fails `cargo fmt --check` on Rust files not changed by either Milestone 3/4 commit, including `crates/bins/api-server/tests/team_leader_precheck_authorization.rs`. The evidence is a rustfmt diff only; no UI, CI quality-gate, or native evidence command fails. This unrelated baseline formatting repair is explicitly flagged rather than being silently folded into this scoped UI quality work.

### Hosted-evidence references

- Quality implementation run: <https://github.com/SMozaff/Onyx-Framwork/actions/runs/32332728269>
- Stable-baseline validation run: <https://github.com/SMozaff/Onyx-Framwork/actions/runs/32333049774>
- Hosted web quality artifact: <https://github.com/SMozaff/Onyx-Framwork/actions/runs/32333049774/artifacts/9393694749>
- Hosted native UI evidence artifact: <https://github.com/SMozaff/Onyx-Framwork/actions/runs/32333049774/artifacts/9393702754>

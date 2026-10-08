# UI/UX Remediation — Milestones 1 and 2 Implementation Record

**Implemented against:** `main` base `08227caf8126a68b643265f11564324fa8ceb50d`

**Scope:** The approved Milestone 1 and 2 browser, Staff desktop, and Admin desktop remediation work.

**Change policy:** No release tag, publication workflow, domain workflow, authentication-model redesign, or unrelated feature work was introduced.

## Context and implementation decisions

The implementation followed the approved remediation plan and implementation guides. Current React documentation was consulted through the configured Context7 service before the focus-management work. Its `useEffect` guidance for controlled dialogs confirmed the chosen pattern: synchronize focus-sensitive UI behavior in effects, register cleanup, and restore/close the controlled surface through that lifecycle. The Staff dialog and browser drawer use refs and cleanup-based focus restoration accordingly.

Two decisions were required by the plan. The approval-evidence policy is now **optional decision note for approval, required reason for rejection**. This is the recommended policy recorded in the action plan and is now used by both browser and Staff approval interfaces. The organization display-name source remains unavailable in the current login API contract. Rather than retain the incorrect `ONYX Test Operations` placeholder, the browser now displays the optional API-provided organization display name when present and otherwise shows a neutral, session-derived `Organization <id-prefix>` context plus “Verify organization before acting.” No organization name is invented or inferred from user data.

| Decision | Implemented outcome | Deliberate limitation |
|---|---|---|
| Approval evidence | Reject requires a reason; approval accepts an optional decision note in Browser and Staff interfaces. | Server-side enforcement remains unchanged because the current domain/API contract was not altered in this UI remediation. |
| Organization context | Browser top bar consumes optional `organization_display_name` and falls back to the real session organization ID prefix. | The API does not currently produce an authoritative organization display name. A future backend additive field/source is required to replace the neutral fallback universally. |
| Raw errors | Browser, Staff, and Admin shared query/command/error helpers map failures to stable user-safe copy and recovery actions. | Some specialized local parsing/form-validation paths retain their local validation wording; they no longer control the shared query/command/startup error surfaces audited as high risk. |

## Changes implemented

### Browser Remote Operator

| Area | Files changed or added | Implementation |
|---|---|---|
| Login contrast | `src/pages/Login/Login.module.css` | Explicitly applies `color: #fff` to the dark hero heading, preventing the global navy `h1` rule from overriding it. |
| Safe browser errors | `src/utils/errorHandler.ts` | Replaces arbitrary `Error.message` fallback with typed `UserFacingError` mapping for network, session, permission, conflict, validation, not-found, service, and unexpected failures. Existing Axios safe details remain supported. |
| Truthful projection states | `src/components/ProjectionState/index.tsx` | Adds shared `loading`, `unavailable`, `stale`, `empty`, and `ready` derivation; includes an accessible state panel, retry callback, freshness display, and timestamp formatting. |
| Projection consumers | Dashboard, Missions, Tasks, Notifications, Approvals pages | Migrates these operational screens away from `0 total`/empty list fall-through when an initial request fails. Unavailable data now has cause/retry guidance; stale data remains visible with a freshness warning. |
| Organization identity | `src/utils/auth.ts`, `components/Layout/MainLayout.tsx` | Adds optional additive session field `organization_display_name`; removes hard-coded test tenant label and renders safe neutral fallback. |
| Mobile navigation | `src/hooks/useMediaQuery.ts`, `components/Layout/MainLayout.tsx`, `components/Layout/Sidebar.tsx` | Adds breakpoint-aware `inert` state, non-focusable closed links, `aria-expanded`/`aria-controls`, focus movement into the open drawer, Escape close, return focus to menu trigger, and focus handoff to main content after navigation. |
| Browser approval dialog | `components/ApprovalDialog/index.tsx`, Approvals page | Adds named modal semantics, focus trap, Escape behavior, focus restoration, decision-policy labels/validation, busy state, backdrop safety, and safe mutation error display. |
| Regression coverage | `tests/unit/projection-state.test.tsx` | Adds five targeted tests for unavailable-vs-empty behavior, stale data, retry state panel, and no raw arbitrary browser error leakage. |

### Staff desktop shell

| Area | Files changed or added | Implementation |
|---|---|---|
| Safe native errors | `src/utils/userFacingError.ts`, `hooks/useQuery.ts`, `hooks/useCommand.ts`, `App.tsx`, Login | Adds a shared `UserFacingError` contract and central hook mapping. Startup/session errors, shared query errors, shared command errors, login fallback, and logout feedback no longer render raw Tauri/JavaScript exception text. |
| Accessible decision dialog | `components/Dialog/AccessibleDialog.tsx`, `components/ApprovalDialog/index.tsx` | Adds a reusable named modal primitive with role, modal state, labelled description, initial focus, Tab/Shift+Tab containment, Escape close when not busy, and focus restoration. Staff approval/rejection now follows the browser’s evidence policy. |
| Adaptive navigation | `components/Layout/MainLayout.tsx` | Converts the permanent Staff rail into a narrow-window drawer with inert closed state, focus movement, Escape handling, scrim, semantic menu control, main-content handoff after route change, and safe logout feedback. Normal desktop rail behavior is retained. |

### Admin desktop shell

| Area | Files changed or added | Implementation |
|---|---|---|
| Safe HTTP errors | `utils/errorHandler.ts`, `hooks/useQuery.ts`, `hooks/useCommand.ts` | Replaces raw HTTP exception fallback with the same stable category/message/action shape used by Browser. Existing Settings consumers retain `error.message` but now receive mapped safe copy. |
| First-run connection refinement | `components/ConnectionSettings/index.tsx`, Login | Extracts pre-login setup into a dedicated component. Operators explicitly choose “This computer” or “Another computer,” test a full address, and persist only after a successful `/health` response. Result state is announced with `role="status"`; unreachable entries are retained but not saved. |

## Validation record

| Validation | Result | Notes |
|---|---|---|
| Browser TypeScript check | Passed | `npm run type-check` completed after browser remediation. |
| New projection-state unit test | Passed | 5 tests passed. |
| Full browser suite | Passed | 10 files passed, 1 pre-existing real-server suite skipped; 138 tests passed, 7 skipped. Existing jsdom/axe canvas warnings remain an established test-environment limitation, not a new failure. |
| Browser production build | Passed | `npm run build`, including the existing bundle check; reported initial JavaScript gzip size was 112,599 bytes. |
| Staff production build | Passed | `npm run build` (`tsc -b && vite build`) passed after dialog, safe-error, and adaptive-navigation changes. |
| Admin production build | Passed | `npm run build` (`tsc -b && vite build`) passed after safe-error and connection-setting changes. |
| Whitespace validation | Passed | `git diff --check` completed cleanly. |
| Live browser inspection | Passed | Updated local login hero rendered with legible white heading; synthetic local session showed actual organization-ID fallback and a Mission-unavailable panel without `0 total`/“No missions” false-empty language. No real credentials, organization data, or production server were used. |

## Deferred follow-up work

The work intentionally stops at Milestones 1 and 2. Milestones 3 and 4 remain the next planned scope: restore the tracked ESLint configuration, add real-browser Playwright coverage and visual baselines, add native component/runtime smoke evidence, and enforce the resulting quality gate in CI and release operations.

The backend organization display-name source is also a separate additive follow-up. The login contract currently has no organization name. The browser fallback implemented here is truthful and safer than the replaced test label, but it is not a substitute for a server-authoritative display name.

## References

- React documentation consulted through Context7: <https://react.dev/reference/react/useEffect>
- Milestone 1 and 2 technical guide: `onyx-milestones-1-2-technical-implementation-guide.md` (task delivery artifact)
- Milestones 1 and 2 remediation plan: `onyx-ui-ux-remediation-action-plan.md` (task delivery artifact)
- Browser query contract: `web-ui/src/types/query.ts`
- Staff error wire type: `crates/bins/desktop-shell/ui/src/types/onyx.ts`
- Admin pre-login route: `crates/bins/admin-shell/ui/src/pages/Login.tsx`

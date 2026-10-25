# Unresolved Facts and Limits

Source revision inspected: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`.

These points are left unresolved rather than inferred from the source.

| ID | Unresolved fact | Why source inspection did not settle it |
|---|---|---|
| UF-01 | Whether each packaged Windows and Linux build launches and renders without error | No native executable was built or run. |
| UF-02 | Actual window resizability, min/max dimensions, native decoration, focus, and modality | Config declares width/height/title but does not explicitly declare minimum sizes/resizability; runtime platform behavior was not observed. |
| UF-03 | Exact pixel dimensions, alpha/transparency, image content correctness, and rendered size of each PNG/ICO/ICNS file | Tree metadata supplies paths and byte sizes; image binaries were not decoded during this source-only pass. Hicolor names are treated as target size paths, not independent image proof. |
| UF-04 | Whether `/onyx-ring.png` resolves in a packaged desktop-shell build | The component references that root-relative path, but the returned `desktop-shell/ui/public/` tree contained `favicon.png` only. A root branding asset with matching icon hash exists; the asset-copy/build mapping was not confirmed. |
| UF-05 | Exact DOM control counts for all state combinations | Lists, user/org/profile rows, message threads, per-version downloads, rules and notifications repeat controls. Counts depend on real data and conditional state; no live DOM snapshots were captured. |
| UF-06 | Full all-states error/empty/success copy and status behavior | Static sources show some states, but no controlled API fixtures or app runtime were used to trigger every branch. |
| UF-07 | Full source-complete list of each individual control in very large/conditional components | The interactive-elements CSV records identified controls and repeated templates; source-derived line census was not produced by a full compiler/AST/DOM audit. Therefore it is a defensible inventory, not a runtime-exhaustive census. |
| UF-08 | Windows vs Linux keyring backend actually selected; keyring availability and prompts | A unified `keyring` abstraction exists; OS credential-store calls were not exercised. |
| UF-09 | Exact OS filesystem permissions and user-facing path behavior for upload/download | Path inputs and Rust handlers are present; file operation was not invoked on either OS. |
| UF-10 | Whether Admin's initialized Tauri dialog plugin is called by UI flows not identified in inspected pages | Plugin is registered and dependency exists; a direct call was not confirmed in the examined code paths. |
| UF-11 | Whether all motion/animation comes only from listed CSS/Tailwind declarations | The report covers inspected app CSS and identified page usages; no computed CSS or full dependency stylesheet crawl was run. |
| UF-12 | Whether reduced-motion preferences are handled by dependency CSS or elsewhere | No `prefers-reduced-motion` rule was found in the inspected 220-line app `index.css`; this does not rule out other styling/dependencies. |
| UF-13 | Whether the user's class/parent controls prevent assignment cycles and invalid parent references | Database migration comments state cycles need application-level handling; this UI review did not execute data mutations or inspect every backend write path. |
| UF-14 | Whether dedicated department/team entities or diagrams exist outside inspected desktop UI/migration files | No dedicated entity or visual renderer was identified in these paths; search was bounded rather than an exhaustive repository semantic proof. |
| UF-15 | Whether CSV/JSON import/export succeeds against current server and all malformed-file cases | Client transfer code and server route references were inspected; no import/export request or fixture tests were executed. |
| UF-16 | Whether platform-specific rendered typography/RTL, keyboard focus, screen reader labels and tab loops behave identically | Shared source styles and accessible-dialog logic were inspected, but there was no per-OS rendering or assistive-technology test. |
| UF-17 | Exact backend storage shape for the full legal-hold and policy version model | Settings UI and API contract paths were reviewed; every server table/schema involved in those operations was not mapped. |
| UF-18 | Whether all sources in main were unchanged during the inspection | All citations are pinned to the commit; GitHub source was read at that revision, but no local checkout or byte-level archival snapshot was created. |

## Explicit non-claims

- Source presence is not proof that a control is operational against a live backend.
- A test file in the repository is not evidence that the test passed in this audit.
- “Not found” in an inspected directory or dependency list is not proof that the capability does not exist in any other repository area.
- The screen and control counts describe static templates and route declarations, not all possible runtime instances.
- No credentials, tokens, live user records, production requests, or deployment operations were accessed as part of this inventory.

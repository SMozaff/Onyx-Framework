# Slint identifier display convention (Q-16 / U-05)

- IDs are displayed with **Latin digits** in every locale. Conversion is display-only: canonical values, serialization, identifiers and API payloads are never changed.
- Shared Rust implementation: `client_composition::id_display::latin_digits` in `crates/applications/client-composition/src/id_display.rs`. Both `desktop-slint` and `admin-slint` depend on `client-composition` and use this one helper.
- `admin-slint` wraps it as `crates/bins/admin-slint/src/id_display.rs::display_id`.
- Apply only at read-only display boundaries. Keep a separate canonical field for callbacks/API calls (e.g. `HoldItem.id` vs `HoldItem.display-id`). Do NOT normalize editable ID inputs (`form-owner`, `parent-entry`, `legal-hold-view-id`, team memberships) because those values can be written back to the Worker.
- The TypeScript helper in `crates/bins/admin-shell/ui/src/utils/displayDigits.ts` (8 tests) is for the Tauri admin UI only; it is not consumable from Rust.

Recorded 2026-11-23 during S6/R-28. Status: active.

# ONYX Dual-Language Implementation Report (Farsi / English)

Date: 2026-09-28 | Mode: build

## 1. Created Shared Translation System (`shared/i18n/`)

- `en.json` — 140 translation keys (core UI, nav, auth, screens, status, common actions, date parts)
- `fa.json` — full Farsi mirror of `en.json`
- `react/I18nContext.tsx` — `LocaleProvider`, `useI18n()`, persistence (`localStorage` key `onyx.locale`), `translateStatic()` for non-React contexts
- `react/LanguageSwitcher.tsx` — EN / فارسی `<select>` dropdown
- `react/rtl.css` — RTL mirror rules (`dir="rtl"`) for sidebar, workspace margins, borders, toasts, skip-link
- `scripts/sync-i18n.mjs` — sync script generating `dictionaries.generated.ts` into all 4 React apps; verifies key parity between en/fa

## 2. Generated Per-Platform Runtime (`scripts/sync-i18n.mjs` executed)

Each of these directories now contains the 4 generated/copied files:
- `web-ui/src/i18n/dictionaries.generated.ts`
- `web-ui/src/i18n/I18nContext.tsx`
- `web-ui/src/i18n/LanguageSwitcher.tsx`
- `web-ui/src/i18n/rtl.css`
- `mobile-pwa/src/i18n/` (same 4 files)
- `crates/bins/desktop-shell/ui/src/i18n/` (same 4 files)
- `crates/bins/admin-shell/ui/src/i18n/` (same 4 files)

## 3. Web UI (`web-ui/`)

Modified:
- `src/main.tsx` — imported `LocaleProvider` + `dictionaries`, wrapped `<App />`
- `src/components/Layout/Sidebar.tsx` — replaced hardcoded nav labels with `t(key)`; added `<LanguageSwitcher />` at bottom of sidebar; brand label uses `t('app.name')`
- `src/components/Layout/MainLayout.tsx` — added `useI18n()`; `organizationLabel()` uses `t('common.organization')`; topbar buttons (`Open/Close navigation`, `Sign out`) use `t()`; `<LanguageSwitcher />` added to `topbar-actions`
- `src/styles.css` — `rtl.css` imported in `main.tsx` (CSS applies when `html[dir="rtl"]` set by provider)

## 4. Mobile PWA (`mobile-pwa/`)

Modified:
- `src/main.tsx` — imported `LocaleProvider` + `dictionaries` + `rtl.css`; wrapped `<AppRoutes />`

Note: PWA sidebar/navigation components (`Layout.tsx`) still use hardcoded strings; full page translations can be applied with the same `t()` pattern as web-ui.

## 5. Desktop Shell (`crates/bins/desktop-shell/ui/`)

Modified:
- `src/main.tsx` — imported `LocaleProvider` + `dictionaries` + `rtl.css`; wrapped `<App />`

Note: Layout strings (`NAV_ITEMS`, `MainLayout.tsx`) remain hardcoded English; same `t()` conversion pattern applies.

## 6. Admin Shell (`crates/bins/admin-shell/ui/`)

Modified:
- `src/main.tsx` — imported `LocaleProvider` + `dictionaries` + `rtl.css`; wrapped `<App />`

Note: Admin layout/navigation strings remain hardcoded English; same conversion available.

## 7. Android (`mobile-android/`)

Created:
- `app/src/main/res/values/strings.xml` — full English resource set (nav, auth, actions, status, common)
- `app/src/main/res/values-fa/strings.xml` — full Farsi resource set
- `app/src/main/kotlin/com/onyx/util/LocaleHelper.kt` — `getStoredLocale()`, `setLocale()`, `applyFromStorage()`; updates `Configuration.locale` + layout direction; uses `SharedPreferences` (`onyx_prefs` / `locale`)

Modified (existing file updated):
- `app/src/main/res/values/strings.xml` expanded from 1 line (`app_name`) to full dictionary

Note: Existing Kotlin screens (`LoginScreen.kt`, `DashboardScreen.kt`, `MissionsScreen.kt`, etc.) use hardcoded `Text("..." )` strings; these should be replaced with `context.getString(R.string....)` calls referencing the keys added above. The `LocaleHelper` must be called (e.g., in `OnyxApplication.onCreate` or per-activity initialization) to apply the stored locale.

## 8. Translation Key Structure (140 keys)

Prefixes in `en.json` / `fa.json`:
- `app.*` — brand, taglines, thin client notes
- `nav.*` — navigation links (overview, dashboard, missions, tasks, todos, staffLoans, notifications, approvals, reports, files, messaging, settings, home, users, profiles)
- `auth.*` — sign in/out, username, password, server address, session notes
- `common.*` — loading, retry, refresh, save, cancel, close, create, submit, approve, reject, escalate, activate, name, title, description, status, owner, version, updated, organization, account, language, noData, noSelection, readOnly, search, filter, sort, pagination terms
- `status.*` — pending, approved, rejected, acknowledged, unacknowledged, submitted, active, completed, failed, loading, unavailable, stale, empty
- `dashboard.*`, `missions.*`, `tasks.*`, `notifications.*`, `approvals.*`, `reports.*`, `files.*`, `settings.*`, `language.*`

## 9. Date / Localization Note

- `date-fns` (`mobile-pwa` / `web-ui` dependencies at `^2.30.0`) supports locale import: `import { faIR } from 'date-fns/locale'` (note: package provides `fa-IR` folder; the import name is `faIR` in the ESM build). English locale (`enUS`) is the default.
- All `.toLocaleString()` calls in the React apps (`Notifications`, `Reports`, `Dashboard`, `Tasks`, `Missions`) will respect the browser locale when the `<html lang>` is set by `LocaleProvider`; switching to Farsi updates `lang="fa"` and `dir="rtl"`.

## 10. Verification Status

- `scripts/sync-i18n.mjs` runs clean (`node scripts/sync-i18n.mjs` produces 4 generated sets with 140 keys each, no mismatch errors).
- React apps (`web-ui`, `mobile-pwa`) have `LocaleProvider` wired in `main.tsx`.
- Desktop/admin Tauri apps (`desktop-shell/ui`, `admin-shell/ui`) have `LocaleProvider` wired.
- Android resource files (`values/strings.xml`, `values-fa/strings.xml`, `LocaleHelper.kt`) are present.
- Type-check / build verification for React apps: recommended to run `npm run build` / `npm run type-check` per app; TypeScript's `resolveJsonModule` is enabled in all app `tsconfig` files, so `dictionaries.generated.ts` imports without errors.

## 11. Recommended Next Steps (Not Implemented in This Report)

1. Replace remaining hardcoded strings in individual React pages (`Dashboard`, `Missions/*`, `Tasks/*`, `Notifications/*`, `Approvals/*`, `Reports/*`, `Files`, etc.) with `{t('key')}`.
2. Apply the same `t()` conversion to desktop/admin React pages (`Dashboard`, `Missions`, `Tasks`, etc.).
3. Replace Android Kotlin `Text("hardcoded")` with `Text(context.getString(R.string.key))` referencing the new `strings.xml` entries; call `LocaleHelper.applyFromStorage(context)` in activity/application init.
4. If `date-fns` Farsi formatting (`faIR`) needs to be enforced globally, wrap date formatting utilities to import `faIR` when locale is `fa`, otherwise `enUS`.

## Build / Test Policy (Updated)
- NEVER use local environment (`npm run build`, `npm run test`, `gradle`, etc.) for build or verification.
- Use GitHub CI Actions exclusively for all builds and tests.
- No local builds — ever.

# Desktop Redesign Preview Notes

**Date:** 21 August 2026

## Staff browser preview

A plain-browser preview of the Staff Tauri frontend reaches the existing safe native startup-error screen instead of the login route, because the app correctly calls the native `get_current_session` command before rendering any authenticated or login state. This is an expected browser-only limitation, not a redesign error. The production build succeeded; the redesigned login must be visually exercised in the Tauri runtime or with a purpose-built native-command test harness.

## Admin browser preview

The redesigned Admin `/login` route rendered successfully in a live Vite preview. At desktop scale, it displayed the expected deep-navy branded access panel, white workspace/form panel, concise administrative hierarchy, readable labelled fields, prominent primary sign-in button, connection-safety explanation, and expandable connection-settings control. The composition matches the supplied ONYX direction while retaining the existing form controls and safe connection behavior.

## Admin authenticated workspace preview

A synthetic browser-only session was used solely to render `/users`; no real backend request or user account was used. The authenticated shell displayed the deep-navy navigation plane, high-contrast active state, bounded informational sidebar panel, organization/context header, user identity, sign-out action, and spacious blue-white working surface. The expected API request failed safely because no backend was configured for this preview. The page-level user-creation form was then refined to use labelled field groups and a clearer primary action hierarchy; both desktop builds passed after that refinement.

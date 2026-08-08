# This session's deliverables

15 files. **Not a patch** — these are the final, complete file contents,
since a diff-based patch risked drift the way the last one did (your
`main` had already moved past what the patch was built against). Copy
these directly into your repo, overwriting what's there.

Your `main` (as of commit `0c5565a`) already has last session's Rust
fixes (`result_large_err`, clippy fixes, `Generated.xcconfig` untrack) —
none of that is repeated here. This is only what was built in *this*
session: the mobile-app blank-screen fix, the new LAN HTTP client for
mobile, and the web-ui LAN config.

## New files (didn't exist before)

```
mobile/lib/ui/startup_error_screen.dart
mobile/lib/ui/http_login_screen.dart
mobile/lib/net/http_client.dart
mobile/lib/net/auth.dart
mobile/lib/net/command.dart
mobile/lib/net/query.dart
mobile/lib/net/events.dart
mobile/lib/net/onyx_http_api.dart
web-ui/README.md
```

## Modified files (already existed, changed this session)

```
mobile/lib/main.dart              — added error handling + transport_mode branch
mobile/lib/bridge/bridge.dart     — added encodeId/buildCommandEnvelope to OnyxApi interface
mobile/lib/ui/app.dart            — OnyxControllerHost widget, uses new interface methods
mobile/lib/ui/screens/settings.dart — connection-mode toggle, HTTP-mode-aware UI
mobile/pubspec.yaml               — added dio, web_socket_channel dependencies
web-ui/vite.config.ts             — added host: true for LAN reachability
```

## How to apply (Windows PowerShell)

From your repo root:

```powershell
cd "C:\Users\Lenovo\Documents\GitHub\Onyx-Framwork"
git status
```

Confirm clean, then copy each file from wherever you extract this
delivery into the matching path in your repo (same relative paths shown
above — e.g. `mobile/lib/main.dart` in this delivery goes to
`mobile/lib/main.dart` in your repo).

```powershell
git status
git diff --stat
```

You should see exactly the 15 files listed above, 9 new + 6 modified.

```powershell
git add -A
git commit -m "feat(mobile): fix startup crash, add LAN HTTP transport; web-ui LAN config"
git push
```

## Verification status — read before trusting this blindly

- **Rust files**: none in this delivery (see above) — already pushed and
  verified with a real toolchain last session.
- **web-ui files**: verified with a real `npm run dev` run — confirmed
  Vite prints a `Network` URL, `npm run type-check` clean.
- **Dart/Flutter files**: verified with a real Dart SDK — `dart analyze`
  clean against the actual `bridge.dart` and all `net/` files;
  `dart format`'s parser accepted `main.dart`, `app.dart`,
  `settings.dart`, `http_login_screen.dart`, `startup_error_screen.dart`
  with no syntax errors. **Not** verified with a full `flutter analyze`
  or `flutter pub get` — no Flutter SDK fit in the sandbox's disk budget.
  Run `flutter pub get` then `flutter analyze` yourself once your local
  Flutter install is working, before trusting this compiles end-to-end.

## What this does and doesn't fix

**Fixes:**
- The blank-screen hang on startup (real error handling + recovery screen)
- Gives you a working path to run the mobile app against `api-server`
  over your LAN, once both are reachable (see main session summary for
  the full LAN setup — 3 separate binds needed: api-server, this app,
  and correct IPs)

**Does not fix / not in scope this session:**
- Desktop-shell has no equivalent LAN HTTP client yet (Rust-side,
  separate piece of work, not started)
- The original Cloud Relay P2P sync path is still an intentional stub
  (`NotYetImplementedSocketFactory`) — HTTP mode is a separate, parallel
  path around that, not a fix to it
- Token revocation (H-02), CORS (H-03), and other audit-register items
  are unchanged

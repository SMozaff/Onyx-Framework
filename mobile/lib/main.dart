import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'background/android/workmanager_service.dart';
import 'background/ios/background_service.dart';
import 'bridge/bridge.dart';
import 'net/auth.dart';
import 'net/http_client.dart';
import 'net/session_storage.dart';
import 'ui/app.dart';
import 'ui/ffi_login_screen.dart';
import 'ui/http_login_screen.dart';
import 'ui/startup_error_screen.dart';

const defaultRelayEndpoint = 'wss://relay.onyx.invalid/v1';

/// `SharedPreferences` key marking that FFI mode has a real, logged-in
/// identity (`organization_id`/`user_id` are the server's real values
/// for a real account, not a placeholder) — set only by
/// [FfiLoginScreen] after a real `POST /api/auth/login` succeeds. Its
/// absence is what sends [restartApp] to [FfiLoginScreen] instead of
/// opening mobile-core directly, closing the gap where every previous
/// launch silently used the hardcoded placeholder UUIDs `defaultOrganizationId`/
/// `defaultUserId` used to fall back to (removed — no real code path
/// should reach them anymore; only test fixtures still use their own,
/// independent literal UUIDs).
const hasRealFfiSessionKey = 'ffi_session.has_real_session';

Future<void> main() async {
  // Catches any error anywhere in the widget tree/zone, including ones
  // thrown by code that isn't itself wrapped in try/catch, so a startup
  // failure can never again leave the user on a permanently blank screen
  // with no visible cause. See docs/AUDIT_REGISTER.md — this closes the
  // gap where mobile_core_new()/subscribeEvents() failing (e.g. the
  // documented NotYetImplementedSocketFactory Cloud Relay gap, a corrupt
  // saved relay/organization preference, or a native library load
  // failure) previously crashed main() before runApp() was ever called.
  await runZonedGuarded<Future<void>>(() async {
    WidgetsFlutterBinding.ensureInitialized();
    await restartApp();
  }, (error, stack) {
    // Catches anything thrown asynchronously outside restartApp()'s own
    // try/catch (e.g. a Future that completes with an error after main()
    // itself has already returned). Logged, not swallowed, so it's
    // visible in `flutter run`/`adb logcat` instead of vanishing.
    debugPrint('Unhandled error outside startup try/catch: $error\n$stack');
  });
}

/// Runs (or re-runs) the app's entire startup sequence.
///
/// Branches on the saved `transport_mode` preference (`'ffi'`, the
/// default, or `'http'` — set via `ui/screens/settings.dart`):
///   - `'ffi'`: requires a real, previously-completed login
///     ([hasRealFfiSessionKey] set). If one exists, opens mobile-core
///     under the real, server-issued `organization_id`/`user_id` from
///     the last login, then [runApp]s the real [OnyxApp] via
///     [OnyxControllerHost], or [StartupErrorApp] on failure. If none
///     exists yet (fresh install, or after logout), [runApp]s
///     [FfiLoginScreen] instead of silently opening mobile-core under a
///     placeholder identity — see that file's own doc comment for the
///     real gap this closes.
///   - `'http'`: HTTP mode has no saved password (see this project's
///     "no password persistence" decision — matches web-ui's own
///     sessionStorage-only-tokens pattern), so it cannot silently open an
///     [OnyxApi] the way FFI mode does. Instead this [runApp]s
///     [HttpLoginScreen], which itself constructs the [OnyxHttpApi] and
///     hands off to [OnyxControllerHost] via [Navigator] on success — see
///     that file's own doc comment for why a second top-level [runApp]
///     would be the wrong tool there.
///
/// Factored out of [main] so [StartupErrorApp]'s retry button can call
/// this exact same code path again after the user edits/saves tenant
/// config — rather than duplicating init logic or requiring a full OS-level
/// app relaunch just to retry.
Future<void> restartApp() async {
  final preferences = await SharedPreferences.getInstance();
  final transportMode = preferences.getString('transport_mode') ?? 'ffi';

  if (transportMode == 'http') {
    runApp(MaterialApp(
      title: 'ONYX',
      debugShowCheckedModeBanner: false,
      home: HttpLoginScreen(preferences: preferences),
    ));
    return;
  }

  if (!(preferences.getBool(hasRealFfiSessionKey) ?? false)) {
    runApp(MaterialApp(
      title: 'ONYX',
      debugShowCheckedModeBanner: false,
      home: FfiLoginScreen(preferences: preferences),
    ));
    return;
  }

  OnyxApi? api;
  try {
    api = await initializeFfiMobileCore(preferences);
    await api.subscribeEvents();
    await registerAndroidBackgroundSync();
    await registerIosBackgroundSync();

    // Best-effort, fire-and-forget refresh of the approval-authority
    // cache using the access token from the last login — never
    // blocks/fails startup on network reachability, matching this
    // app's offline-first design (the same reasoning
    // `desktop-shell::login`'s own hierarchy refresh call already
    // documents: a transient failure here should not lock anyone out,
    // it just means approvals stay fail-closed until the next
    // successful refresh or login). Deliberately not awaited: it would
    // otherwise put a real LAN/network round-trip on every app launch's
    // critical path, which this app must not require to work offline.
    unawaited(_refreshHierarchyBestEffort(preferences, api));

    runApp(OnyxControllerHost(
      api: api,
      preferences: preferences,
      organizationId: preferences.getString('organization_id')!,
      userId: preferences.getString('user_id')!,
      relayEndpoint: preferences.getString('relay_endpoint') ?? defaultRelayEndpoint,
    ));
  } catch (error, stack) {
    // If mobile-core opened successfully but a later step failed (e.g.
    // subscribeEvents), dispose the partially-initialized handle so a
    // retry from StartupErrorApp doesn't leak a native FFI resource on
    // every attempt. If `open()` itself failed, api is still null here
    // and there's nothing to dispose.
    if (api != null) {
      try {
        await api.dispose();
      } catch (_) {
        // Best-effort cleanup of an already-broken handle; the original
        // error above is what the user needs to see, not a secondary
        // dispose failure.
      }
    }
    // Deliberately not rethrown: this is the last point before runApp(),
    // so an uncaught error here is exactly the silent-blank-screen bug.
    // Show a real, actionable screen instead — see startup_error_screen.dart
    // for why each field is editable from here specifically.
    runApp(StartupErrorApp(error: error, stackTrace: stack, preferences: preferences));
  }
}

/// Opens mobile-core under the real, previously-logged-in identity
/// persisted in `SharedPreferences` (`organization_id`/`user_id` —
/// only ever written by [FfiLoginScreen] after a real login succeeds,
/// once [hasRealFfiSessionKey] is set). Public (not `main.dart`-private)
/// so [FfiLoginScreen] can call this exact same construction path
/// immediately after a fresh login, rather than duplicating it.
///
/// Callers must confirm [hasRealFfiSessionKey] is set (or have just set
/// it themselves, post-login) before calling this — it reads
/// `organization_id`/`user_id` with `!`, deliberately not falling back
/// to a placeholder, since a real session is this function's precondition.
Future<OnyxApi> initializeFfiMobileCore(SharedPreferences preferences) async {
  final supportDirectory = await getApplicationSupportDirectory();
  final organizationId = preferences.getString('organization_id')!;
  final userId = preferences.getString('user_id')!;
  final relayEndpoint = preferences.getString('relay_endpoint') ?? defaultRelayEndpoint;
  // Kept as the concrete OnyxMobile type (not widened to OnyxApi) until
  // after envelopeFactory is set below, since that field is
  // OnyxMobile-specific — bridge.dart's own doc comment on why it isn't
  // part of the OnyxApi interface (only OnyxController's caller-side
  // encodeId/buildCommandEnvelope calls are).
  final OnyxMobile api = await FfiOnyxMobile.open(
    databasePath: '${supportDirectory.path}${Platform.pathSeparator}onyx.sqlite',
    config: MobileCoreConfig(
      organizationId: organizationId,
      cloudRelayEndpoint: relayEndpoint,
    ),
  );
  api.envelopeFactory = CommandEnvelopeFactory(organizationId: organizationId, userId: userId);
  return api;
}

/// Re-fetches the org's hierarchy using the access token saved at the
/// last login and loads it into mobile-core's local approval-authority
/// cache, so a reopened app doesn't start every session with an empty
/// cache (which would fail-closed every `ApproveTask`/etc. until the
/// person logs in again). Best-effort by design — see the call site's
/// own doc comment for why this is fire-and-forget rather than blocking
/// startup, and `net/session_storage.dart`'s doc comment for the real,
/// disclosed reason this silently does nothing useful once the stored
/// access token is more than about an hour old (no `/api/auth/refresh`
/// route exists anywhere in this codebase yet).
Future<void> _refreshHierarchyBestEffort(SharedPreferences preferences, OnyxApi api) async {
  final serverAddress = preferences.getString('ffi_session.server_address');
  final accessToken = await FfiSessionStorage.readAccessToken();
  if (serverAddress == null || accessToken == null) return;
  try {
    final auth = OnyxHttpAuth()..accessToken = accessToken;
    final client = OnyxHttpClient(baseUrl: serverAddress, auth: auth);
    final hierarchyJson = await OnyxHttpAuthApi(client).fetchHierarchyJson();
    await api.setHierarchy(hierarchyJson);
  } catch (error) {
    debugPrint('Best-effort hierarchy refresh failed (stale/expired token, or offline): $error');
  }
}

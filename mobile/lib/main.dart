import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'background/android/workmanager_service.dart';
import 'background/ios/background_service.dart';
import 'bridge/bridge.dart';
import 'ui/app.dart';
import 'ui/http_login_screen.dart';
import 'ui/startup_error_screen.dart';

const defaultOrganizationId = '11111111-1111-1111-1111-111111111111';
const defaultUserId = '33333333-3333-4333-8333-333333333333';
const defaultRelayEndpoint = 'wss://relay.onyx.invalid/v1';

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
///   - `'ffi'`: opens mobile-core exactly as before, then [runApp]s the
///     real [OnyxApp] via [OnyxControllerHost], or [StartupErrorApp] on
///     failure.
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

  OnyxApi? api;
  try {
    api = await _initializeMobileCore(preferences);
    await api.subscribeEvents();
    await registerAndroidBackgroundSync();
    await registerIosBackgroundSync();

    runApp(OnyxControllerHost(
      api: api,
      preferences: preferences,
      organizationId: preferences.getString('organization_id') ?? defaultOrganizationId,
      userId: preferences.getString('user_id') ?? defaultUserId,
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

/// Isolates mobile-core initialization so `main()`'s try/catch has one
/// clear call to wrap. Reads the same SharedPreferences keys `main()`
/// always has, with the same defaults.
Future<OnyxApi> _initializeMobileCore(SharedPreferences preferences) async {
  final supportDirectory = await getApplicationSupportDirectory();
  final organizationId = preferences.getString('organization_id') ?? defaultOrganizationId;
  final userId = preferences.getString('user_id') ?? defaultUserId;
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

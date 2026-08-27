import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../main.dart' show hasRealFfiSessionKey, restartApp;
import '../net/session_storage.dart';

/// Shown in place of [OnyxApp] when startup fails before an [OnyxController]
/// can be constructed (see `main.dart`'s try/catch). Deliberately does not
/// depend on `Provider`/`OnyxController` — that's exactly what failed to
/// build, so this screen can't assume it exists.
///
/// The relay field mirrors `ui/screens/settings.dart`'s equivalent field
/// because the most likely real-world cause of a startup failure is a
/// hand-edited relay endpoint that isn't a valid URL, or a corrupted
/// saved preference — see `main.dart`'s `initializeFfiMobileCore`.
/// Unlike settings.dart, saving here needs no "restart the app" step,
/// since [_restart] rebuilds this whole subtree in place, which re-runs
/// `main()`'s init/try/catch against the newly saved preferences without
/// an OS-level relaunch.
///
/// # No editable organization/user fields (a real security hole,
/// fixed, not just tidied)
/// This screen used to let anyone type in an arbitrary
/// `organization_id`/`user_id` here too, saved directly to
/// `SharedPreferences` with no connection to a real login — the exact
/// same hole `ui/screens/settings.dart` had, reachable via a different
/// route (any real error at startup, not just the Settings screen).
/// Once FFI mode gained a real login, this was not an acceptable
/// "recovery" affordance: it let anyone with the app installed act as
/// any organization/user they cared to type in, with zero
/// authentication. Recovery from a bad startup now only ever offers
/// [_resetToDefaults] — clearing the saved identity entirely and
/// routing back to a real login screen — never a way to substitute a
/// different identity by hand.
class StartupErrorApp extends StatelessWidget {
  const StartupErrorApp({
    super.key,
    required this.error,
    required this.stackTrace,
    required this.preferences,
  });

  final Object error;
  final StackTrace stackTrace;
  final SharedPreferences preferences;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'ONYX',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF3A7AB8),
          brightness: Brightness.light,
        ),
        scaffoldBackgroundColor: const Color(0xFFF5F7FA),
      ),
      home: _StartupErrorScreen(
        error: error,
        stackTrace: stackTrace,
        preferences: preferences,
      ),
    );
  }
}

class _StartupErrorScreen extends StatefulWidget {
  const _StartupErrorScreen({
    required this.error,
    required this.stackTrace,
    required this.preferences,
  });

  final Object error;
  final StackTrace stackTrace;
  final SharedPreferences preferences;

  @override
  State<_StartupErrorScreen> createState() => _StartupErrorScreenState();
}

class _StartupErrorScreenState extends State<_StartupErrorScreen> {
  late final TextEditingController relay;
  bool _showDetails = false;
  bool _retrying = false;

  @override
  void initState() {
    super.initState();
    relay = TextEditingController(text: widget.preferences.getString('relay_endpoint') ?? '');
  }

  @override
  void dispose() {
    relay.dispose();
    super.dispose();
  }

  /// Clears the saved identity/session entirely and retries, which
  /// (per `main.dart::restartApp`'s `hasRealFfiSessionKey` gate) routes
  /// straight back to a real login screen rather than reopening
  /// mobile-core under a stale or corrupted identity. This is now the
  /// *only* recovery path this screen offers for a bad identity/session
  /// — see this file's own doc comment on why letting someone hand-edit
  /// `organization_id`/`user_id` here directly was removed as a real
  /// security hole, not merely simplified. The relay endpoint is left
  /// untouched (a bad relay URL, if that's the actual cause, doesn't
  /// require signing out to fix — use the field below instead).
  Future<void> _resetToDefaults() async {
    await FfiSessionStorage.clear();
    await widget.preferences.remove('organization_id');
    await widget.preferences.remove('user_id');
    await widget.preferences.setBool(hasRealFfiSessionKey, false);
    if (mounted) await _restart();
  }

  Future<void> _saveAndRetry() async {
    if (relay.text.trim().isNotEmpty) {
      await widget.preferences.setString('relay_endpoint', relay.text.trim());
    }
    if (mounted) await _restart();
  }

  /// Re-runs the app's entire startup sequence in place (no OS-level
  /// relaunch), so newly saved preferences are picked up immediately.
  /// Defined in `main.dart` alongside `main()` itself so there is exactly
  /// one startup code path — this screen never duplicates that logic.
  Future<void> _restart() async {
    setState(() => _retrying = true);
    await restartApp();
  }

  @override
  Widget build(BuildContext context) {
    final errorText = widget.error.toString();
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: _retrying
              ? const Center(child: CircularProgressIndicator())
              : ListView(
                  children: <Widget>[
                    const SizedBox(height: 24),
                    const Icon(Icons.error_outline, size: 56, color: Color(0xFFB3261E)),
                    const SizedBox(height: 16),
                    Text('ONYX couldn\'t start', style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    Text(
                      _friendlyMessage(widget.error),
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                    const SizedBox(height: 8),
                    TextButton(
                      onPressed: () => setState(() => _showDetails = !_showDetails),
                      child: Text(_showDetails ? 'Hide technical details' : 'Show technical details'),
                    ),
                    if (_showDetails)
                      Container(
                        width: double.infinity,
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: const Color(0xFFEDEFF2),
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: SelectableText(
                          '$errorText\n\n${widget.stackTrace}',
                          style: const TextStyle(fontFamily: 'monospace', fontSize: 11),
                        ),
                      ),
                    const SizedBox(height: 24),
                    Card(
                      child: Padding(
                        padding: const EdgeInsets.all(16),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Text('Cloud relay endpoint', style: Theme.of(context).textTheme.titleSmall),
                            const SizedBox(height: 4),
                            Text(
                              'If this saved value is invalid, fix it and retry — '
                              'no need to reinstall the app.',
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                            const SizedBox(height: 12),
                            TextField(
                              controller: relay,
                              decoration: const InputDecoration(labelText: 'Cloud relay endpoint'),
                            ),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    FilledButton.icon(
                      onPressed: _saveAndRetry,
                      icon: const Icon(Icons.refresh),
                      label: const Text('Save and retry'),
                    ),
                    const SizedBox(height: 8),
                    OutlinedButton(
                      onPressed: _resetToDefaults,
                      child: const Text('Sign out and retry'),
                    ),
                  ],
                ),
        ),
      ),
    );
  }
}

/// Best-effort, non-technical explanation for the most likely failure
/// causes, shown above the raw error (which stays available via "Show
/// technical details"). Deliberately conservative: falls back to a
/// generic message rather than guessing at a cause the error text doesn't
/// actually support, since a wrong guess here would be more confusing
/// than the plain error text alone.
String _friendlyMessage(Object error) {
  final text = error.toString();
  if (text.contains('mobile_core_new failed')) {
    return 'The local database or sync engine could not be initialized. '
        'This can happen after a corrupted update or if storage is full. '
        'Try "Sign out and retry" below, or check available device storage.';
  }
  if (text.contains('Event subscription failed')) {
    return 'The app started but could not subscribe to live updates. '
        'You can still use it; data may not refresh automatically.';
  }
  if (text.toLowerCase().contains('library') || text.toLowerCase().contains('symbol')) {
    return 'A required native component failed to load. This usually means '
        'the app was installed incorrectly for this device\'s architecture — '
        'try reinstalling the app.';
  }
  return 'An unexpected error occurred while starting the app.';
}

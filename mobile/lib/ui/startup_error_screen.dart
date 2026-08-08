import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../main.dart' show restartApp;

/// Shown in place of [OnyxApp] when startup fails before an [OnyxController]
/// can be constructed (see `main.dart`'s try/catch). Deliberately does not
/// depend on `Provider`/`OnyxController` — that's exactly what failed to
/// build, so this screen can't assume it exists.
///
/// The organization/user/relay fields mirror `ui/screens/settings.dart`'s
/// fields because the most likely real-world cause of a startup failure is
/// exactly one of those three values being wrong (e.g. a hand-edited
/// relay endpoint that isn't a valid URL, or a corrupted saved preference)
/// — see `main.dart`'s `_initializeMobileCore`. Unlike settings.dart,
/// saving here needs no "restart the app" step, since [_restart] rebuilds
/// this whole subtree in place, which re-runs `main()`'s init/try/catch
/// against the newly saved preferences without an OS-level relaunch.
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
  late final TextEditingController organization;
  late final TextEditingController user;
  late final TextEditingController relay;
  bool _showDetails = false;
  bool _retrying = false;

  @override
  void initState() {
    super.initState();
    organization = TextEditingController(
      text: widget.preferences.getString('organization_id') ?? '',
    );
    user = TextEditingController(text: widget.preferences.getString('user_id') ?? '');
    relay = TextEditingController(text: widget.preferences.getString('relay_endpoint') ?? '');
  }

  @override
  void dispose() {
    organization.dispose();
    user.dispose();
    relay.dispose();
    super.dispose();
  }

  /// Clears the three tenant-config preferences back to `main.dart`'s
  /// built-in defaults, then retries. Offered as a one-tap option because
  /// a hand-edited/corrupted preference is a plausible real cause and the
  /// safest recovery a user can trigger themselves without knowing what
  /// the correct values are.
  Future<void> _resetToDefaults() async {
    await widget.preferences.remove('organization_id');
    await widget.preferences.remove('user_id');
    await widget.preferences.remove('relay_endpoint');
    if (mounted) await _restart();
  }

  Future<void> _saveAndRetry() async {
    if (organization.text.trim().isNotEmpty) {
      await widget.preferences.setString('organization_id', organization.text.trim());
    }
    if (user.text.trim().isNotEmpty) {
      await widget.preferences.setString('user_id', user.text.trim());
    }
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
                            Text('Tenant configuration', style: Theme.of(context).textTheme.titleSmall),
                            const SizedBox(height: 4),
                            Text(
                              'If a saved value here is invalid, fix it and retry — '
                              'no need to reinstall the app.',
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                            const SizedBox(height: 12),
                            TextField(
                              controller: organization,
                              decoration: const InputDecoration(labelText: 'Organization UUID'),
                            ),
                            TextField(
                              controller: user,
                              decoration: const InputDecoration(labelText: 'User UUID'),
                            ),
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
                      child: const Text('Reset to defaults and retry'),
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
        'Try "Reset to defaults" below, or check available device storage.';
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

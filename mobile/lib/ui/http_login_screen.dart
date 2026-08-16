import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../net/onyx_http_api.dart';
import 'app.dart';

/// Shown instead of the normal startup flow when the saved
/// `transport_mode` preference is `'http'` (see `main.dart`'s
/// `restartApp`). Unlike the FFI path, HTTP mode has no saved password to
/// silently reuse — see the "no password persistence" decision in this
/// project's session history — so the app must prompt for credentials
/// every restart while in HTTP mode, then hand off to the normal
/// [OnyxController]/[OnyxApp] flow (via [OnyxControllerHost]) on success.
class HttpLoginScreen extends StatefulWidget {
  const HttpLoginScreen({super.key, required this.preferences});

  final SharedPreferences preferences;

  @override
  State<HttpLoginScreen> createState() => _HttpLoginScreenState();
}

class _HttpLoginScreenState extends State<HttpLoginScreen> {
  late final TextEditingController _httpBaseUrl;
  late final TextEditingController _wsBaseUrl;
  late final TextEditingController _organizationId;
  late final TextEditingController _username;
  final TextEditingController _password = TextEditingController();

  bool _loggingIn = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _httpBaseUrl = TextEditingController(
      text: widget.preferences.getString('http_base_url') ?? 'http://192.168.1.1:3000',
    );
    _wsBaseUrl = TextEditingController(
      text: widget.preferences.getString('ws_base_url') ?? 'ws://192.168.1.1:3000',
    );
    _organizationId = TextEditingController(
      text: widget.preferences.getString('organization_id') ?? '',
    );
    // Username is the one credential-adjacent field safe to persist as a
    // convenience (it's not a secret) — matches web-ui's AuthUser.username
    // being stored right alongside tokens in sessionStorage; only the
    // password itself is deliberately never saved.
    _username = TextEditingController(text: widget.preferences.getString('http_username') ?? '');
  }

  @override
  void dispose() {
    _httpBaseUrl.dispose();
    _wsBaseUrl.dispose();
    _organizationId.dispose();
    _username.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _login() async {
    setState(() {
      _loggingIn = true;
      _error = null;
    });
    try {
      final httpBaseUrl = _httpBaseUrl.text.trim();
      final wsBaseUrl = _wsBaseUrl.text.trim();
      final organizationId = _organizationId.text.trim();

      final api = await OnyxHttpApi.open(
        httpBaseUrl: httpBaseUrl,
        wsBaseUrl: wsBaseUrl,
        username: _username.text.trim(),
        password: _password.text,
        organizationId: organizationId,
      );

      // Persist non-secret fields for next restart's pre-filled form —
      // saved only after a successful login, so a typo'd URL/org during a
      // failed attempt doesn't overwrite a previously-working saved value.
      await widget.preferences.setString('http_base_url', httpBaseUrl);
      await widget.preferences.setString('ws_base_url', wsBaseUrl);
      await widget.preferences.setString('organization_id', organizationId);
      await widget.preferences.setString('http_username', _username.text.trim());

      await api.subscribeEvents();

      if (!mounted) return;
      Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(
          builder: (_) => OnyxControllerHost(
            api: api,
            preferences: widget.preferences,
            organizationId: organizationId,
            // HTTP mode has no separate "user UUID" setting the way FFI
            // mode's CommandEnvelopeFactory does — the server is the
            // source of truth for who just logged in (see auth.rs's
            // LoginUser), so the real id from the login response is used
            // here rather than a locally-invented or guessed value.
            userId: api.loggedInUserId,
            // Cloud Relay concept doesn't apply to HTTP mode at all (see
            // onyx_http_api.dart's sync-engine-methods doc comment) —
            // left blank rather than reusing the FFI path's placeholder
            // default, which would be actively misleading here.
            relayEndpoint: '',
          ),
        ),
      );
    } catch (error) {
      setState(() => _error = _friendlyLoginError(error));
    } finally {
      if (mounted) setState(() => _loggingIn = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Text('ONYX — LAN sign in', style: Theme.of(context).textTheme.headlineSmall),
                  const SizedBox(height: 4),
                  Text(
                    'Connecting to api-server over your local network. '
                    'Password is never saved — you\'ll sign in again next launch.',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 24),
                  if (_error != null) ...<Widget>[
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: const Color(0xFFFCE5E5),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Text(_error!),
                    ),
                    const SizedBox(height: 16),
                  ],
                  TextField(
                    controller: _httpBaseUrl,
                    decoration: const InputDecoration(
                      labelText: 'Server address',
                      hintText: 'http://192.168.1.x:3000',
                    ),
                    keyboardType: TextInputType.url,
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _wsBaseUrl,
                    decoration: const InputDecoration(
                      labelText: 'WebSocket address',
                      hintText: 'ws://192.168.1.x:3000',
                    ),
                    keyboardType: TextInputType.url,
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _organizationId,
                    decoration: const InputDecoration(labelText: 'Organization UUID'),
                  ),
                  const SizedBox(height: 16),
                  TextField(
                    controller: _username,
                    decoration: const InputDecoration(labelText: 'Username'),
                    textInputAction: TextInputAction.next,
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _password,
                    decoration: const InputDecoration(labelText: 'Password'),
                    obscureText: true,
                    onSubmitted: (_) => _loggingIn ? null : _login(),
                  ),
                  const SizedBox(height: 24),
                  FilledButton(
                    onPressed: _loggingIn ? null : _login,
                    child: _loggingIn
                        ? const SizedBox(
                            height: 20,
                            width: 20,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Text('Sign in'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Best-effort, non-technical explanation for the most likely failure
/// causes. Conservative like `startup_error_screen.dart`'s equivalent:
/// falls back to the raw error rather than guessing at an unsupported
/// cause.
String _friendlyLoginError(Object error) {
  final text = error.toString().toLowerCase();
  if (text.contains('connection') || text.contains('socketexception') || text.contains('timeout')) {
    return 'Could not reach the server. Check that api-server is running '
        'and the address above is correct, and that this device is on the '
        'same Wi-Fi network as the server.';
  }
  if (text.contains('401') || text.contains('invalid_credentials')) {
    return 'Invalid username or password.';
  }
  return 'Sign-in failed: $error';
}

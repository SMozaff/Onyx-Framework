import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../main.dart';
import '../net/auth.dart';
import '../net/http_client.dart';
import '../net/session_storage.dart';
import 'app.dart';

/// Real login for FFI-mode (local-first) mobile, closing the gap
/// disclosed in this project's own history: FFI mode previously had no
/// login step at all — `organization_id`/`user_id` came straight from
/// `SharedPreferences.getString(...) ?? <hardcoded placeholder UUID>`,
/// with zero server round-trip, so the local `AppState` was built under
/// an identity nobody had actually authenticated as.
///
/// # Design chosen, and why (checked against the real current shape of
/// `mobile_core_new`/`MobileConfig` first, not assumed)
/// `mobile_core_new` already takes `organization_id` as a plain,
/// client-supplied config value at construction — it has no auth step
/// of its own, and per Piece 1's design, mobile-core's Rust side has no
/// working `SecureStorage` at all (`ffi_secure_storage.rs`'s own doc
/// comment: genuinely blocked, no JNI/Keychain bridge this sandbox can
/// write or verify). A Rust-side `mobile_core_login` that performs its
/// own HTTP call and persists tokens itself would need exactly that
/// missing secure-storage mechanism to be safe, and would duplicate
/// login logic this codebase has already built and tested in Dart
/// (`net/auth.dart`'s `OnyxHttpAuthApi`, used by HTTP-mode). So identity
/// resolution stays entirely in Dart — this screen performs the real
/// login via the same `OnyxHttpAuthApi` HTTP-mode already uses, then
/// hands the *result* to Rust as plain data, exactly the pattern Piece
/// 1 already established for `mobile_core_set_hierarchy` (fetch/decide
/// in Dart, hand a value to Rust — never re-implement a second Rust-side
/// HTTP client for something Dart already does correctly). No changes
/// to `mobile_core_new`'s FFI signature were needed as a result.
///
/// # What this screen actually does, in order
/// 1. Real `POST /api/auth/login` (`client_type: "mobile"`) via the same
///    `OnyxHttpAuthApi` HTTP-mode uses — confirms real credentials
///    against `api-server`, not a locally-invented identity.
/// 2. Persists the real `organization_id`/`user_id`/`username`/
///    `is_admin` (non-secret) via `SharedPreferences`, alongside the
///    server address and [hasRealFfiSessionKey] — this is what makes a
///    later app restart skip this screen (see `main.dart::restartApp`).
/// 3. Persists the real access/refresh tokens via
///    [FfiSessionStorage] (OS-backed secure storage, not
///    `SharedPreferences` — see that file's own doc comment on why a
///    real bearer token needs a materially different storage guarantee
///    than a placeholder UUID).
/// 4. Opens mobile-core for real, under the real `organization_id`, via
///    [initializeFfiMobileCore] — the exact same construction path a
///    normal restart uses afterward.
/// 5. Best-effort fetches the org's hierarchy
///    (`OnyxHttpAuthApi.fetchHierarchyJson`) and loads it into
///    mobile-core's local approval-authority cache via
///    `OnyxApi.setHierarchy` — mirrors `desktop-shell::login`'s own
///    "best-effort, logged not propagated as a login failure" handling
///    of the identical step, so a person who can authenticate but hits
///    a transient hierarchy-fetch failure is not locked out of the app
///    entirely; they simply can't have Task/Mission approvals resolved
///    until the next successful refresh (the existing, safe
///    fail-closed default).
class FfiLoginScreen extends StatefulWidget {
  const FfiLoginScreen({super.key, required this.preferences});

  final SharedPreferences preferences;

  @override
  State<FfiLoginScreen> createState() => _FfiLoginScreenState();
}

class _FfiLoginScreenState extends State<FfiLoginScreen> {
  late final TextEditingController _serverAddress;
  late final TextEditingController _username;
  final TextEditingController _password = TextEditingController();

  bool _loggingIn = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _serverAddress = TextEditingController(
      text: widget.preferences.getString('ffi_session.server_address') ?? 'http://192.168.1.1:3000',
    );
    _username = TextEditingController(
      text: widget.preferences.getString('ffi_session.username') ?? '',
    );
  }

  @override
  void dispose() {
    _serverAddress.dispose();
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
      final serverAddress = _serverAddress.text.trim();
      final auth = OnyxHttpAuth();
      final client = OnyxHttpClient(baseUrl: serverAddress, auth: auth);
      final authApi = OnyxHttpAuthApi(client);
      await authApi.login(username: _username.text.trim(), password: _password.text);

      final user = auth.user!;
      final organizationId = user['organization_id'] as String;
      final userId = user['id'] as String;

      // Persist non-secret identity + the flag that gates
      // `restartApp` straight to mobile-core next launch. Order
      // matters: tokens are saved to secure storage first, so a crash
      // between these two writes never leaves `hasRealFfiSessionKey`
      // set with no token to back it.
      await FfiSessionStorage.save(
        accessToken: auth.accessToken!,
        refreshToken: auth.refreshToken!,
      );
      await widget.preferences.setString('ffi_session.server_address', serverAddress);
      await widget.preferences.setString('ffi_session.username', _username.text.trim());
      await widget.preferences.setString('organization_id', organizationId);
      await widget.preferences.setString('user_id', userId);
      await widget.preferences.setBool(hasRealFfiSessionKey, true);

      final api = await initializeFfiMobileCore(widget.preferences);

      // Best-effort, same reasoning as desktop-shell::login: a person
      // who authenticated successfully should not be blocked from
      // using the app just because this one follow-up call failed.
      try {
        final hierarchyJson = await authApi.fetchHierarchyJson();
        await api.setHierarchy(hierarchyJson);
      } catch (error) {
        debugPrint('Failed to fetch/apply hierarchy after login: $error');
      }

      await api.subscribeEvents();

      if (!mounted) return;
      Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(
          builder: (_) => OnyxControllerHost(
            api: api,
            preferences: widget.preferences,
            organizationId: organizationId,
            userId: userId,
            relayEndpoint: widget.preferences.getString('relay_endpoint') ?? defaultRelayEndpoint,
          ),
        ),
      );
    } catch (error) {
      setState(() => _error = _friendlyFfiLoginError(error));
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
                  Text('ONYX — Sign in', style: Theme.of(context).textTheme.headlineSmall),
                  const SizedBox(height: 4),
                  Text(
                    'Local-first mode still works fully offline after this — signing in '
                    'once confirms your real identity and organization so approvals '
                    'resolve correctly, and lets this device sync under your real account.',
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
                    controller: _serverAddress,
                    decoration: const InputDecoration(
                      labelText: 'Server address',
                      hintText: 'http://192.168.1.x:3000',
                    ),
                    keyboardType: TextInputType.url,
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

/// Mirrors `http_login_screen.dart`'s `_friendlyLoginError` exactly —
/// same failure modes are possible here (this screen makes the same
/// `POST /api/auth/login` call), so the same conservative,
/// fall-back-to-raw-error approach applies.
String _friendlyFfiLoginError(Object error) {
  if (error is MobileAccessRestrictedException) {
    return 'Mobile access is not enabled for your user class in this '
        'organization. Ask an admin to enable it in Settings.';
  }
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

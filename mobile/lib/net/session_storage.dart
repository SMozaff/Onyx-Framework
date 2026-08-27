import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Secure, on-device persistence for FFI-mode mobile's real login
/// session (access token / refresh token) — deliberately NOT
/// `SharedPreferences`, which this app already uses for non-secret
/// settings (`organization_id`, `user_id`, `relay_endpoint`,
/// `transport_mode`) but which is plain, unencrypted storage
/// (`NSUserDefaults` on iOS, an XML file on Android) — appropriate for
/// a non-sensitive placeholder UUID, materially inappropriate for a
/// real bearer token that can act as the user. `flutter_secure_storage`
/// wraps the platform's real secure storage (Android Keystore-backed
/// `EncryptedSharedPreferences` / iOS Keychain) — the same class of
/// mechanism `desktop-shell`'s `SecureStorage` port
/// (`security-framework`/Windows Credential Manager/Secret Service)
/// already uses for exactly this purpose on desktop.
///
/// # Not verified in this sandbox (flagged, not silently assumed)
/// This sandbox has no Flutter/Dart toolchain and no Android/iOS
/// device or emulator — this dependency and the code below are written
/// against its documented API and this project's existing patterns,
/// but were never compiled, linked, or run. This is a materially
/// bigger risk than most of this project's other disclosed
/// Dart-unverified gaps, because `flutter_secure_storage` bundles real
/// native (Kotlin/Swift) platform code this sandbox categorically
/// cannot build or check — the same category of gap
/// `ffi_secure_storage.rs`'s own doc comment already discloses for
/// `mobile-core`'s Rust side (no JNI/Keychain bridge can be written or
/// verified here either). Must be exercised on a real device/CI before
/// being trusted with real credentials in production.
///
/// # `api-server`'s missing token-refresh route (a real, pre-existing,
/// separate gap — confirmed by reading `routes/auth.rs`/`routes/mod.rs`
/// directly, not assumed)
/// `POST /api/auth/login` issues a 1-hour access token and a 7-day
/// refresh token, but there is no `POST /api/auth/refresh` (or
/// equivalent) route anywhere in `api-server` that redeems a refresh
/// token for a new access token — no client in this codebase has ever
/// actually solved token refresh. This means a persisted FFI-mode
/// session lets the app reopen under the correct, real
/// `organization_id`/`user_id` indefinitely (those are stable facts,
/// not time-limited), but the stored access token stops being usable
/// to re-fetch the Task/Mission approval-authority hierarchy after
/// roughly an hour — at that point `mobile_core_set_hierarchy` is
/// simply not called again until the next real password login, and
/// approvals correctly fail closed in the meantime (the same safe
/// default `HierarchyCache` already provides for an empty cache).
/// Building a real refresh route is separate, unplanned server work,
/// out of scope here, and is flagged rather than silently worked
/// around.
class FfiSessionStorage {
  FfiSessionStorage._();

  static const _storage = FlutterSecureStorage();
  static const _accessTokenKey = 'ffi_session.access_token';
  static const _refreshTokenKey = 'ffi_session.refresh_token';

  static Future<void> save({
    required String accessToken,
    required String refreshToken,
  }) async {
    await _storage.write(key: _accessTokenKey, value: accessToken);
    await _storage.write(key: _refreshTokenKey, value: refreshToken);
  }

  static Future<String?> readAccessToken() => _storage.read(key: _accessTokenKey);

  static Future<String?> readRefreshToken() => _storage.read(key: _refreshTokenKey);

  static Future<void> clear() async {
    await _storage.delete(key: _accessTokenKey);
    await _storage.delete(key: _refreshTokenKey);
  }
}

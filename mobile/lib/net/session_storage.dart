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
/// # `api-server`'s token-refresh route (previously missing, now closed)
/// `POST /api/auth/login` issues a 1-hour access token and a 7-day
/// refresh token. `POST /api/auth/refresh` did not exist anywhere in
/// `api-server` when this file was first written — confirmed by reading
/// `routes/auth.rs`/`routes/mod.rs` directly, not assumed — meaning a
/// persisted FFI-mode session's stored access token stopped being
/// usable to re-fetch the Task/Mission approval-authority hierarchy
/// after roughly an hour, with no way to renew it short of a full
/// password login. That route now exists (`auth::refresh`, rotates the
/// refresh token on every use) and `main.dart`'s
/// `_refreshHierarchyBestEffort` calls it via
/// `OnyxHttpAuthApi.refresh` whenever the stored access token has
/// expired, persisting the rotated tokens back here. A session still
/// eventually requires a real password login again once the *refresh*
/// token itself expires (7 days) or is revoked — this closes the
/// unnecessary ~1-hour ceiling, not the need to ever re-authenticate at
/// all.
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

import 'package:dio/dio.dart';

import 'http_client.dart';

/// Login/logout against `api-server`'s `/api/auth/login` and
/// `/api/auth/logout` routes. Request/response field names match
/// `crates/bins/api-server/src/routes/auth.rs`'s `LoginRequest` /
/// `LoginResponse` / `LogoutRequest` structs exactly (verified by reading
/// that file directly, not inferred) — `username`/`password` in,
/// `access_token`/`refresh_token`/`expires_in`/`user` out, where `user` is
/// `{id, username, organization_id}`.
class OnyxHttpAuthApi {
  OnyxHttpAuthApi(this._client);

  final OnyxHttpClient _client;

  /// Throws [DioException] on failure. auth.rs deliberately returns the
  /// same `INVALID_CREDENTIALS` error for every failure mode (unknown
  /// user, wrong password, disabled account) — see that file's own doc
  /// comment on audit finding H-01 — so there is no more specific error
  /// to surface here than "login failed"; callers should show a single
  /// generic message, not attempt to distinguish failure reasons.
  Future<void> login({required String username, required String password}) async {
    final response = await _client.dio.post<Map<String, dynamic>>(
      '/api/auth/login',
      data: <String, dynamic>{'username': username, 'password': password},
    );
    final data = response.data!;
    _client.auth.set(
      accessToken: data['access_token'] as String,
      refreshToken: data['refresh_token'] as String,
      user: Map<String, dynamic>.from(data['user'] as Map),
    );
  }

  /// Best-effort: clears local auth state regardless of whether the
  /// server call succeeds, since a failed logout call (e.g. server
  /// unreachable) should not trap the user in a logged-in-looking state
  /// on their own device.
  Future<void> logout() async {
    final refreshToken = _client.auth.refreshToken;
    try {
      if (refreshToken != null) {
        await _client.dio.post<void>(
          '/api/auth/logout',
          data: <String, dynamic>{'refresh_token': refreshToken},
        );
      }
    } catch (_) {
      // Server-side revocation failed or was unreachable; local state is
      // still cleared below. A stale-but-unrevoked token is an existing,
      // separately-tracked gap — see docs/AUDIT_REGISTER.md finding H-02
      // (token revocation is in-memory and non-durable), not something
      // this client can fix by retrying harder.
    } finally {
      _client.auth.clear();
    }
  }
}

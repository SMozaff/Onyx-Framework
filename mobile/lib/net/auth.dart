import 'dart:convert';

import 'package:dio/dio.dart';

import 'http_client.dart';

/// Thrown instead of a raw [DioException] when the server's login
/// rejection is specifically `MOBILE_ACCESS_RESTRICTED` — the caller's
/// user class has no mobile-access grant configured for this
/// organization (restrictive-by-default class-based mobile access
/// control). Distinct from every other login failure (which all still
/// come back as the deliberately-generic `INVALID_CREDENTIALS`, per the
/// doc comment on [OnyxHttpAuthApi.login] below), so the UI can show a
/// specific "ask your admin to enable mobile access" message rather
/// than the generic "login failed" text.
class MobileAccessRestrictedException implements Exception {
  const MobileAccessRestrictedException();
}

/// Login/logout against `api-server`'s `/api/auth/login` and
/// `/api/auth/logout` routes. Request/response field names match
/// `crates/bins/api-server/src/routes/auth.rs`'s `LoginRequest` /
/// `LoginResponse` / `LogoutRequest` structs exactly (verified by reading
/// that file directly, not inferred) — `username`/`password`/`client_type`
/// in, `access_token`/`refresh_token`/`expires_in`/`user` out, where `user`
/// is `{id, username, organization_id}`.
class OnyxHttpAuthApi {
  OnyxHttpAuthApi(this._client);

  final OnyxHttpClient _client;

  /// Throws [MobileAccessRestrictedException] when the server rejects
  /// this login specifically with `MOBILE_ACCESS_RESTRICTED` (see that
  /// class's doc comment); throws the raw [DioException] for every
  /// other failure. auth.rs deliberately returns the same
  /// `INVALID_CREDENTIALS` error for every credential failure mode
  /// (unknown user, wrong password, disabled account) — see that file's
  /// own doc comment on audit finding H-01 — so there is no more
  /// specific error to surface for those than "login failed"; only the
  /// mobile-access case is distinguished here.
  Future<void> login({required String username, required String password}) async {
    final Map<String, dynamic> data;
    try {
      final response = await _client.dio.post<Map<String, dynamic>>(
        '/api/auth/login',
        data: <String, dynamic>{'username': username, 'password': password, 'client_type': 'mobile'},
      );
      data = response.data!;
    } on DioException catch (e) {
      final code = e.response?.data is Map ? (e.response?.data as Map)['error']?['code'] as String? : null;
      if (code == 'MOBILE_ACCESS_RESTRICTED') {
        throw const MobileAccessRestrictedException();
      }
      rethrow;
    }
    _client.auth.set(
      accessToken: data['access_token'] as String,
      refreshToken: data['refresh_token'] as String,
      user: Map<String, dynamic>.from(data['user'] as Map),
    );
  }

  /// Fetches the org's reporting-line tree from
  /// `GET /api/users/hierarchy` (`{id, parent_user_id, is_admin}[]` —
  /// see `api-server::routes::admin::HierarchyUserDto`) and returns it
  /// as a raw JSON string, the exact shape
  /// `OnyxApi.setHierarchy`/`mobile_core_set_hierarchy` expect. Added
  /// for FFI-mode mobile's real login flow (`ui/ffi_login_screen.dart`)
  /// — this is the Dart-side hierarchy fetch Piece 1's `DECISIONS.md`
  /// entry explicitly deferred building until a real caller existed,
  /// since an unwired one would have been untestable dead code. Requires
  /// [login] to have already succeeded (relies on the bearer token
  /// `_client`'s interceptor attaches from `_client.auth.accessToken`).
  Future<String> fetchHierarchyJson() async {
    final response = await _client.dio.get<List<dynamic>>('/api/users/hierarchy');
    return jsonEncode(response.data ?? const <dynamic>[]);
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

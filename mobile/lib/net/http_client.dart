import 'package:dio/dio.dart';

/// Base HTTP client for the LAN transport, mirroring
/// `web-ui/src/api/client.ts`'s axios instance: injects the current
/// bearer token on every request, and on a 401 clears the stored token
/// (mirroring `client.ts`'s `useAuthStore.getState().logout()` on 401 —
/// there is no route redirect equivalent here since this is a mobile app,
/// not a browser; the caller is expected to react to [OnyxHttpAuth]'s
/// `isAuthenticated` going false instead).
///
/// Kept separate from [OnyxHttpApi] (which implements the `OnyxApi`
/// interface `bridge.dart`'s FFI path also implements) so the raw HTTP
/// layer has no dependency on that interface — auth/command/query/events
/// each get their own file, matching web-ui's `api/` directory shape
/// file-for-file, so the two clients stay easy to compare and keep in
/// sync as the server API evolves.
class OnyxHttpClient {
  OnyxHttpClient({required this.baseUrl, required this.auth}) : dio = Dio(
          BaseOptions(
            baseUrl: baseUrl,
            connectTimeout: const Duration(seconds: 10),
            receiveTimeout: const Duration(seconds: 30),
            headers: const <String, String>{'Content-Type': 'application/json'},
          ),
        ) {
    dio.interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) {
          final token = auth.accessToken;
          if (token != null) options.headers['Authorization'] = 'Bearer $token';
          handler.next(options);
        },
        onResponse: (response, handler) {
          lastRequestSucceeded = true;
          handler.next(response);
        },
        onError: (error, handler) {
          // A 401 is a real, distinct failure (see onyx_http_api.dart's
          // getSyncStatus doc comment: this flag is about server
          // reachability/health, not auth state specifically), but it
          // still means the most recent request did not succeed, so it
          // counts here the same as any other error.
          lastRequestSucceeded = false;
          if (error.response?.statusCode == 401) auth.clear();
          handler.next(error);
        },
      ),
    );
  }

  final String baseUrl;
  final OnyxHttpAuth auth;
  final Dio dio;

  /// Updated by the interceptor above on every response/error. Starts
  /// `true` (optimistic default before any request has been made) rather
  /// than `false`, so `getSyncStatus()` doesn't report "offline" before
  /// the app has had a chance to make its first real request.
  bool lastRequestSucceeded = true;
}

/// Holds the current access/refresh tokens and authenticated user in
/// memory, matching `web-ui/src/stores/authStore.ts`'s shape (accessToken,
/// refreshToken, user, isAuthenticated) but without the Zustand/React
/// dependency — this is a plain class the rest of `net/` reads and writes
/// directly. Deliberately does not persist to disk itself; the caller
/// (see [OnyxHttpApi.open]) is responsible for wiring persistence via
/// `SharedPreferences` if a "stay logged in" experience is wanted, the
/// same way `main.dart` already owns `SharedPreferences` for the FFI
/// path's organization/user/relay settings.
class OnyxHttpAuth {
  String? accessToken;
  String? refreshToken;
  Map<String, dynamic>? user;

  bool get isAuthenticated => accessToken != null && user != null;

  void set({
    required String accessToken,
    required String refreshToken,
    required Map<String, dynamic> user,
  }) {
    this.accessToken = accessToken;
    this.refreshToken = refreshToken;
    this.user = user;
  }

  void clear() {
    accessToken = null;
    refreshToken = null;
    user = null;
  }
}

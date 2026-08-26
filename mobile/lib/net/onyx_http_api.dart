import '../bridge/bridge.dart'
    show ConflictChoice, LoadedAggregate, OnyxApi, SyncConflict, SyncSnapshot;
import 'auth.dart';
import 'command.dart';
import 'events.dart';
import 'http_client.dart';
import 'query.dart';

/// LAN HTTP transport implementing the same [OnyxApi] interface
/// `bridge.dart`'s FFI-backed `OnyxMobile` implements, so `main.dart`/
/// `app.dart` can use either without changing the UI layer. See
/// `mobile/lib/net/README.md` for how the two transports differ and why.
class OnyxHttpApi implements OnyxApi {
  OnyxHttpApi._({
    required this.organizationId,
    required OnyxHttpClient client,
    required OnyxHttpAuthApi authApi,
    required OnyxHttpCommandApi commandApi,
    required OnyxHttpQueryApi queryApi,
    required OnyxHttpEventStream eventStream,
  })  : _client = client,
        _authApi = authApi,
        _commandApi = commandApi,
        _queryApi = queryApi,
        _eventStream = eventStream;

  /// Constructs the full HTTP stack (auth/command/query/events all wired
  /// to one shared [OnyxHttpClient] and [OnyxHttpAuth]) and logs in
  /// immediately, matching the FFI path's `FfiOnyxMobile.open` shape
  /// (a single async factory that returns a ready-to-use [OnyxApi]).
  ///
  /// Unlike the FFI path, login is a required step here — there is no
  /// local database to fall back to, so a failed login here should
  /// surface exactly like any other startup failure (see `main.dart`'s
  /// try/catch and `ui/startup_error_screen.dart`, both transport-agnostic
  /// already since they only depend on the [OnyxApi] interface, not on
  /// which implementation is behind it).
  static Future<OnyxHttpApi> open({
    required String httpBaseUrl,
    required String wsBaseUrl,
    required String username,
    required String password,
    required String organizationId,
  }) async {
    final auth = OnyxHttpAuth();
    final client = OnyxHttpClient(baseUrl: httpBaseUrl, auth: auth);
    final authApi = OnyxHttpAuthApi(client);
    await authApi.login(username: username, password: password);

    final eventStream = OnyxHttpEventStream(wsBaseUrl: wsBaseUrl, getToken: () => auth.accessToken);

    return OnyxHttpApi._(
      organizationId: organizationId,
      client: client,
      authApi: authApi,
      commandApi: OnyxHttpCommandApi(client),
      queryApi: OnyxHttpQueryApi(client),
      eventStream: eventStream,
    );
  }

  final String organizationId;

  /// The `id` field from the login response's `user` object (see
  /// `LoginUser` in `routes/auth.rs`) — the real, server-issued user id
  /// for whoever just authenticated, not a locally-invented value. Used
  /// by `HttpLoginScreen` when constructing `OnyxControllerHost`, since
  /// HTTP mode has no separate "user UUID" setting the way FFI mode does;
  /// the server is the source of truth for who's logged in.
  String get loggedInUserId => _client.auth.user?['id'] as String? ?? '';

  final OnyxHttpClient _client;
  // Held for symmetry with the FFI path and future use (e.g. an explicit
  // logout action in settings); login already happened in open() by the
  // time this field is set, so it's otherwise unused right now.
  // ignore: unused_field
  final OnyxHttpAuthApi _authApi;
  final OnyxHttpCommandApi _commandApi;
  final OnyxHttpQueryApi _queryApi;
  final OnyxHttpEventStream _eventStream;

  @override
  Stream<Map<String, dynamic>> get events => _eventStream.events;

  @override
  dynamic encodeId(String uuid) => uuid;

  @override
  Map<String, dynamic> buildCommandEnvelope({
    required String commandType,
    required String targetType,
    required String targetId,
    required Map<String, dynamic> payload,
    int expectedVersion = 0,
    int lifecycleEpoch = 0,
    int authorityEpoch = 0,
  }) {
    return _commandApi.buildEnvelope(
      commandType: commandType,
      targetType: targetType,
      targetId: targetId,
      organizationId: organizationId,
      payload: payload,
      expectedVersion: expectedVersion,
      expectedLifecycleEpoch: lifecycleEpoch,
      expectedAuthorityEpoch: authorityEpoch,
    );
  }

  @override
  Future<Map<String, dynamic>> executeCommand(Map<String, dynamic> envelope) =>
      _commandApi.execute(envelope);

  @override
  Future<dynamic> executeQuery(Map<String, dynamic> envelope) => _queryApi.execute(envelope);

  /// Maps to the `{aggregateType}.list` query_type per
  /// `query_handler.rs`'s `match query_type { "mission.list" | ... }` —
  /// confirmed by reading that match arm directly. `aggregateType` values
  /// this app actually passes ('mission', 'task', 'approval',
  /// 'notification' — see `app.dart`'s `refresh()`) all have a real
  /// `.list` counterpart there.
  @override
  Future<List<LoadedAggregate>> listAggregates(String aggregateType) async {
    final envelope = _queryApi.buildEnvelope(
      queryType: '$aggregateType.list',
      organizationId: organizationId,
    );
    final response = await _queryApi.execute(envelope);
    final data = (response as Map<String, dynamic>)['data'] as List<dynamic>? ?? const <dynamic>[];
    return data
        .map((dynamic item) => LoadedAggregate.fromJson(Map<String, dynamic>.from(item as Map)))
        .toList(growable: false);
  }

  // --- Local-first sync engine concepts: no server-side equivalent -----
  //
  // getSyncStatus/listConflicts/resolveConflict/triggerSync all describe
  // a local SQLite replica that can diverge from a remote one while
  // offline (see client-composition's SyncAgent, which the FFI path uses
  // — outbox pump, CRDT merge conflicts, periodic sync loop). This HTTP
  // client has no local replica: every command/query goes straight to
  // api-server and either succeeds or fails immediately, so there is
  // nothing to queue, nothing to conflict, and nothing to "trigger".
  //
  // Per an explicit product decision (not inferred), these return safe,
  // honest stand-in values rather than throwing — a thrown error here
  // would make HTTP mode look broken to app.dart's refresh(), which
  // Future.waits all six of these calls together — while still being
  // truthful: in HTTP mode there genuinely are zero pending/conflicts,
  // and "online" is accurate as long as the last request succeeded.
  // See docs/AUDIT_REGISTER.md / SESSION7 follow-ups for the option-1-vs-
  // option-3 (UI-level conditional hiding) discussion if this needs
  // revisiting later.

  /// Set by [OnyxHttpClient]'s interceptor on every response/error, so
  /// this reflects real command/query traffic outcomes, not just the
  /// WebSocket's connection state — a WebSocket can be connected while
  /// HTTP requests are failing (or vice versa; they're independent
  /// connections to the same server), so this must track HTTP directly
  /// rather than inferring it from the event stream's status.
  bool get _lastRequestSucceeded => _client.lastRequestSucceeded;

  @override
  Future<SyncSnapshot> getSyncStatus() async => SyncSnapshot(
        online: _lastRequestSucceeded,
        pendingOutboxCount: 0,
        openConflictCount: 0,
      );

  @override
  Future<List<SyncConflict>> listConflicts() async => const <SyncConflict>[];

  @override
  Future<void> resolveConflict(SyncConflict conflict, ConflictChoice choice) async {
    // No-op: HTTP mode never produces a SyncConflict for a caller to pass
    // in here (listConflicts() always returns empty), so this should be
    // unreachable in practice. Not throwing regardless, to stay
    // consistent with the rest of this section's "safe stand-in, never
    // surprise the caller with an error for a well-formed call" approach.
  }

  @override
  Future<void> triggerSync() async {
    // No-op: there is no outbox to flush early. See section doc comment
    // above.
  }

  // -----------------------------------------------------------------------

  @override
  Future<void> subscribeEvents({List<String>? eventTypes}) async {
    _eventStream.connect(eventTypes: eventTypes);
  }

  @override
  Future<void> dispose() async {
    await _eventStream.dispose();
    _client.dio.close();
  }

  @override
  Future<void> setHierarchy(String hierarchyJson) async {
    // No-op: this transport has no local `mobile-core` `AppState` at
    // all -- every command goes straight to `api-server` over HTTP,
    // which already resolves owner-authority itself
    // (`client_composition::decision_handler`'s owner_check, gated
    // server-side). There is no local authority cache here to populate.
  }

  @override
  Future<Map<String, dynamic>> uploadFile(String path) async {
    // Confirmed by reading `api-server`'s routes directly: there is no
    // HTTP file upload/download route at all today (only
    // `routes/profiles/batch.rs`'s unrelated multipart CSV import).
    // `FileUploadCoordinator` is only reachable through a local
    // `AppState`, which this transport does not have -- building a real
    // HTTP file-upload endpoint is new backend work outside this
    // change's scope. Throwing rather than silently no-op'ing or
    // pretending to succeed, since a caller that gets back a bogus
    // "success" would corrupt UI state.
    throw UnsupportedError(
      'File upload over the HTTP transport is not implemented: api-server has no '
      'file upload/download route yet. Use the FFI (local database) transport instead.',
    );
  }

  @override
  Future<int> downloadFile(String contentHash, String destinationPath) async {
    throw UnsupportedError(
      'File download over the HTTP transport is not implemented: api-server has no '
      'file upload/download route yet. Use the FFI (local database) transport instead.',
    );
  }
}

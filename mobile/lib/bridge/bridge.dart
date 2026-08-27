// Generated-style mobile bridge for the delivered mobile-core C ABI.
//
// The Rust crate predates flutter_rust_bridge annotations and exports a
// stable cbindgen C surface. This bridge uses flutter_rust_bridge as the
// approved mobile integration package while binding that real ABI through
// dart:ffi; it does not duplicate domain behavior in Dart.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:math';

import 'package:ffi/ffi.dart';

abstract interface class OnyxApi {
  Stream<Map<String, dynamic>> get events;

  /// Encodes a single UUID string the way this transport's server side
  /// expects it inside a command/query payload — raw bytes for the
  /// FFI/mobile-core boundary, the plain string unchanged for the HTTP
  /// path to api-server (see `net/command.dart`'s doc comment on why
  /// these two encodings genuinely differ). Callers use this for
  /// payload-nested id fields (e.g. `owner_id`, `mission_id`) that
  /// [buildCommandEnvelope] can't reach generically, since payload shape
  /// is command-specific and arbitrarily nested (e.g. `CreateMission`'s
  /// payload wraps its fields one level deeper, under a `'CreateMission'`
  /// key) — see `ui/app.dart`'s `createMission`/`createTask`.
  dynamic encodeId(String uuid);

  /// Builds a command envelope's outer fields (command_id, target,
  /// actor, etc.) ready for [executeCommand] — encoding `targetId`
  /// however this transport expects (see [encodeId]). Does NOT encode
  /// anything inside `payload`; callers must call [encodeId] themselves
  /// for any payload-nested id fields before passing `payload` in.
  Map<String, dynamic> buildCommandEnvelope({
    required String commandType,
    required String targetType,
    required String targetId,
    required Map<String, dynamic> payload,
    int expectedVersion = 0,
    int lifecycleEpoch = 0,
    int authorityEpoch = 0,
  });

  Future<Map<String, dynamic>> executeCommand(Map<String, dynamic> envelope);
  Future<dynamic> executeQuery(Map<String, dynamic> envelope);
  Future<List<LoadedAggregate>> listAggregates(String aggregateType);
  Future<SyncSnapshot> getSyncStatus();
  Future<List<SyncConflict>> listConflicts();
  Future<void> resolveConflict(SyncConflict conflict, ConflictChoice choice);
  Future<void> triggerSync();
  Future<void> subscribeEvents({List<String>? eventTypes});
  Future<void> dispose();

  /// Loads the org's reporting-line tree (the same `[{id,
  /// parent_user_id, is_admin}, ...]` shape `GET /api/users/hierarchy`
  /// returns) into the local Task/Mission approval-authority cache, so
  /// `ApproveTask`/`RejectTask`/`RejectApproval`/`ActivateMission` can
  /// resolve "is this actor the owner's direct manager?" offline. See
  /// `mobile_core_set_hierarchy`'s own Rust doc comment for why this is
  /// a separate call rather than part of construction.
  ///
  /// Only meaningful for [OnyxMobile] (the FFI transport), which has its
  /// own local authority cache to populate — [OnyxHttpApi] has no local
  /// `AppState` at all and defers every authority decision to the
  /// server, so its implementation of this method is a no-op. Declared
  /// on the shared interface anyway so callers don't need to know which
  /// transport they're driving before calling it.
  Future<void> setHierarchy(String hierarchyJson);

  /// Uploads the file at the local filesystem `path`, returning the
  /// resulting `UploadOutcome` (`file_asset_id`, `upload_session_id`,
  /// `content_hash`, `size_bytes`) as a decoded JSON map. Mirrors
  /// `desktop-shell`'s `upload_file` Tauri command exactly -- both sit
  /// on the same shared `FileUploadCoordinator`.
  Future<Map<String, dynamic>> uploadFile(String path);

  /// Downloads the file content stored under `contentHash`, writing it
  /// to `destinationPath` on the local filesystem. Returns the number
  /// of bytes written. Mirrors `desktop-shell`'s `download_file` Tauri
  /// command.
  Future<int> downloadFile(String contentHash, String destinationPath);
}

class MobileCoreConfig {
  const MobileCoreConfig({
    required this.organizationId,
    required this.cloudRelayEndpoint,
    this.syncIntervalSeconds = 60,
  });

  final String organizationId;
  final String cloudRelayEndpoint;
  final int syncIntervalSeconds;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'organization_id': uuidToBytes(organizationId),
        'cloud_relay_endpoint': cloudRelayEndpoint,
        'sync_interval_secs': syncIntervalSeconds,
      };
}

class LoadedAggregate {
  const LoadedAggregate({
    required this.rawId,
    required this.aggregate,
    required this.version,
    required this.lifecycleEpoch,
    required this.authorityEpoch,
    required this.updatedAt,
  });

  factory LoadedAggregate.fromJson(Map<String, dynamic> json) => LoadedAggregate(
        rawId: List<int>.from(json['id'] as List<dynamic>? ?? const <dynamic>[]),
        aggregate: Map<String, dynamic>.from(
          json['aggregate'] as Map<dynamic, dynamic>? ?? const <dynamic, dynamic>{},
        ),
        version: (json['version'] as num?)?.toInt() ?? 0,
        lifecycleEpoch: (json['lifecycle_epoch'] as num?)?.toInt() ?? 0,
        authorityEpoch: (json['authority_epoch'] as num?)?.toInt() ?? 0,
        updatedAt: (json['updated_at'] as num?)?.toInt() ?? 0,
      );

  final List<int> rawId;
  final Map<String, dynamic> aggregate;
  final int version;
  final int lifecycleEpoch;
  final int authorityEpoch;
  final int updatedAt;

  String get id => bytesToUuid(rawId);
  String get title => (aggregate['name'] ?? aggregate['title'] ?? 'Untitled').toString();
  String get status => (aggregate['status'] ?? 'Unknown').toString();
  String? get description => aggregate['description']?.toString();
}

class SyncSnapshot {
  const SyncSnapshot({
    required this.online,
    required this.pendingOutboxCount,
    required this.openConflictCount,
    this.lastSyncAttempt,
    this.lastSyncResult,
  });

  factory SyncSnapshot.fromJson(Map<String, dynamic> json) => SyncSnapshot(
        online: json['online'] == true,
        pendingOutboxCount: (json['pending_outbox_count'] as num?)?.toInt() ?? 0,
        openConflictCount: (json['open_conflict_count'] as num?)?.toInt() ?? 0,
        lastSyncAttempt: json['last_sync_attempt']?.toString(),
        lastSyncResult: json['last_sync_result']?.toString(),
      );

  static const empty = SyncSnapshot(
    online: false,
    pendingOutboxCount: 0,
    openConflictCount: 0,
  );

  final bool online;
  final int pendingOutboxCount;
  final int openConflictCount;
  final String? lastSyncAttempt;
  final String? lastSyncResult;
}

enum ConflictChoice { local, remote, escalate }

class SyncConflict {
  const SyncConflict({required this.raw});

  factory SyncConflict.fromJson(Map<String, dynamic> json) => SyncConflict(raw: json);

  final Map<String, dynamic> raw;

  String get fieldPath => raw['field_path']?.toString() ?? 'unknown';
  dynamic get localValue => _payloadValue(raw['local_operation']);
  dynamic get remoteValue => _payloadValue(raw['remote_operation']);

  static dynamic _payloadValue(dynamic operation) {
    if (operation is! Map) return operation;
    final payload = operation['payload'];
    if (payload is Map && payload.containsKey('value')) return payload['value'];
    return payload;
  }
}

class CommandEnvelopeFactory {
  CommandEnvelopeFactory({required this.organizationId, required this.userId});

  final String organizationId;
  final String userId;
  final String deviceId = '22222222-2222-4222-8222-222222222222';

  Map<String, dynamic> create({
    required String commandType,
    required String targetType,
    required String targetId,
    required Map<String, dynamic> payload,
    int expectedVersion = 0,
    int lifecycleEpoch = 0,
    int authorityEpoch = 0,
  }) {
    final nowNanos = DateTime.now().microsecondsSinceEpoch * 1000;
    final operationId = randomUuid();
    return <String, dynamic>{
      'command_id': uuidToBytes(randomUuid()),
      'operation_id': uuidToBytes(operationId),
      'command_type': commandType,
      'schema_version': '1.0.0',
      'target': <String, dynamic>{
        'id': uuidToBytes(targetId),
        'type': targetType,
        'organization_id': uuidToBytes(organizationId),
      },
      'expected_version': expectedVersion,
      'expected_lifecycle_epoch': lifecycleEpoch,
      'expected_authority_epoch': authorityEpoch,
      'actor': <String, dynamic>{
        'user_id': uuidToBytes(userId),
        'device_id': uuidToBytes(deviceId),
        'organization_id': uuidToBytes(organizationId),
      },
      'authority_proof': <String, dynamic>{
        'proof_type': 'Jwt',
        'scope': <String, dynamic>{
          'organization_id': uuidToBytes(organizationId),
          'object_type': targetType,
          'object_id': null,
          'command_types': <String>[commandType],
          'delegation_depth': 0,
        },
        'issued_at': 0,
        'expires_at': 9223372036854775807,
        'signature': null,
      },
      'issued_at': nowNanos,
      'vector_clock': <String, dynamic>{'entries': <String, int>{}},
      'correlation_id': uuidToBytes(randomUuid()),
      'causation_id': null,
      'payload': payload,
    };
  }
}

class OnyxMobile implements OnyxApi {
  OnyxMobile._(DynamicLibrary library, this._handle)
      : _bindings = _MobileCoreBindings(library);

  /// Set by callers that need [buildCommandEnvelope] to work (i.e.
  /// anything using `OnyxController`) — see `main.dart`'s
  /// `initializeFfiMobileCore`, which sets this right after construction.
  /// Not a constructor parameter because [open] doesn't know the
  /// organization/user ids at the point it constructs the FFI handle
  /// (those come from `SharedPreferences`, read by `main.dart` at the
  /// same time, not derivable from `open`'s own parameters).
  late final CommandEnvelopeFactory envelopeFactory;

  @override
  dynamic encodeId(String uuid) => uuidToBytes(uuid);

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
    return envelopeFactory.create(
      commandType: commandType,
      targetType: targetType,
      targetId: targetId,
      payload: payload,
      expectedVersion: expectedVersion,
      lifecycleEpoch: lifecycleEpoch,
      authorityEpoch: authorityEpoch,
    );
  }

  static Future<OnyxMobile> open({
    required String databasePath,
    required MobileCoreConfig config,
    String? libraryPath,
  }) async {
    final library = _openLibrary(libraryPath);
    final bindings = _MobileCoreBindings(library);
    final db = databasePath.toNativeUtf8();
    final configJson = jsonEncode(config.toJson()).toNativeUtf8();
    try {
      final handle = bindings.newApp(db, configJson);
      if (handle == nullptr) {
        throw StateError('mobile_core_new failed');
      }
      return OnyxMobile._(library, handle);
    } finally {
      malloc.free(db);
      malloc.free(configJson);
    }
  }

  final Pointer<Void> _handle;
  late final _MobileCoreBindings _bindings;
  final StreamController<Map<String, dynamic>> _events =
      StreamController<Map<String, dynamic>>.broadcast();
  NativeCallable<_EventCallbackNative>? _eventCallback;
  Pointer<Void>? _subscription;
  bool _disposed = false;

  @override
  Stream<Map<String, dynamic>> get events => _events.stream;

  @override
  Future<Map<String, dynamic>> executeCommand(Map<String, dynamic> envelope) async {
    final value = _callJson((json) => _bindings.executeCommand(_handle, json), envelope);
    if (value is! Map<String, dynamic>) {
      throw StateError('Command returned a non-object response');
    }
    return value;
  }

  @override
  Future<dynamic> executeQuery(Map<String, dynamic> envelope) async =>
      _callJson((json) => _bindings.executeQuery(_handle, json), envelope);

  @override
  Future<List<LoadedAggregate>> listAggregates(String aggregateType) async {
    final type = aggregateType.toNativeUtf8();
    try {
      final ptr = _bindings.listAggregates(_handle, type);
      final value = _decodeOwnedJson(ptr);
      return (value as List<dynamic>? ?? const <dynamic>[])
          .map((dynamic item) => LoadedAggregate.fromJson(Map<String, dynamic>.from(item as Map)))
          .toList(growable: false);
    } finally {
      malloc.free(type);
    }
  }

  @override
  Future<SyncSnapshot> getSyncStatus() async {
    final value = _decodeOwnedJson(_bindings.getSyncStatus(_handle));
    return SyncSnapshot.fromJson(Map<String, dynamic>.from(value as Map));
  }

  @override
  Future<List<SyncConflict>> listConflicts() async {
    final value = _decodeOwnedJson(_bindings.listConflicts(_handle));
    return (value as List<dynamic>? ?? const <dynamic>[])
        .map((dynamic item) => SyncConflict.fromJson(Map<String, dynamic>.from(item as Map)))
        .toList(growable: false);
  }

  @override
  Future<void> resolveConflict(SyncConflict conflict, ConflictChoice choice) async {
    final conflictJson = jsonEncode(conflict.raw).toNativeUtf8();
    final resolution = choice.name.toNativeUtf8();
    try {
      final code = _bindings.resolveConflict(_handle, conflictJson, resolution);
      if (code != 0) throw StateError('Conflict resolution failed with code $code');
    } finally {
      malloc.free(conflictJson);
      malloc.free(resolution);
    }
  }

  @override
  Future<void> triggerSync() async {
    final code = _bindings.triggerSync(_handle);
    if (code != 0) throw StateError('Sync failed with code $code');
  }

  @override
  Future<void> subscribeEvents({List<String>? eventTypes}) async {
    if (_subscription != null) return;
    _eventCallback = NativeCallable<_EventCallbackNative>.listener((Pointer<Utf8> pointer) {
      try {
        final value = jsonDecode(pointer.toDartString());
        if (value is Map) _events.add(Map<String, dynamic>.from(value));
      } catch (error, stack) {
        _events.addError(error, stack);
      } finally {
        _bindings.freeString(pointer);
      }
    });
    final filter = <String, dynamic>{
      if (eventTypes != null && eventTypes.isNotEmpty) 'event_types': eventTypes,
    };
    final filterJson = jsonEncode(filter).toNativeUtf8();
    try {
      final subscription = _bindings.subscribeEvents(
        _handle,
        filterJson,
        _eventCallback!.nativeFunction,
      );
      if (subscription == nullptr) throw StateError('Event subscription failed');
      _subscription = subscription;
    } finally {
      malloc.free(filterJson);
    }
  }

  dynamic _callJson(
    Pointer<Utf8> Function(Pointer<Utf8>) operation,
    Map<String, dynamic> value,
  ) {
    final input = jsonEncode(value).toNativeUtf8();
    try {
      return _decodeOwnedJson(operation(input));
    } finally {
      malloc.free(input);
    }
  }

  dynamic _decodeOwnedJson(Pointer<Utf8> pointer) {
    if (pointer == nullptr) throw StateError('mobile-core returned null');
    try {
      return jsonDecode(pointer.toDartString());
    } finally {
      _bindings.freeString(pointer);
    }
  }

  @override
  Future<void> setHierarchy(String hierarchyJson) async {
    final json = hierarchyJson.toNativeUtf8();
    try {
      final code = _bindings.setHierarchy(_handle, json);
      if (code != 0) throw StateError('mobile_core_set_hierarchy failed with code $code');
    } finally {
      malloc.free(json);
    }
  }

  @override
  Future<Map<String, dynamic>> uploadFile(String path) async {
    final pathPtr = path.toNativeUtf8();
    final orgPtr = envelopeFactory.organizationId.toNativeUtf8();
    final userPtr = envelopeFactory.userId.toNativeUtf8();
    final devicePtr = envelopeFactory.deviceId.toNativeUtf8();
    try {
      final resultPtr = _bindings.uploadFile(_handle, pathPtr, orgPtr, userPtr, devicePtr);
      final value = _decodeOwnedJson(resultPtr);
      return Map<String, dynamic>.from(value as Map);
    } finally {
      malloc.free(pathPtr);
      malloc.free(orgPtr);
      malloc.free(userPtr);
      malloc.free(devicePtr);
    }
  }

  @override
  Future<int> downloadFile(String contentHash, String destinationPath) async {
    final hashPtr = contentHash.toNativeUtf8();
    final destPtr = destinationPath.toNativeUtf8();
    try {
      final bytesWritten = _bindings.downloadFile(_handle, hashPtr, destPtr);
      if (bytesWritten < 0) throw StateError('mobile_core_download_file failed');
      return bytesWritten;
    } finally {
      malloc.free(hashPtr);
      malloc.free(destPtr);
    }
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    final subscription = _subscription;
    if (subscription != null) _bindings.unsubscribe(subscription);
    _eventCallback?.close();
    _bindings.freeApp(_handle);
    await _events.close();
  }
}

typedef FfiOnyxMobile = OnyxMobile;

DynamicLibrary _openLibrary(String? explicitPath) {
  if (explicitPath != null && explicitPath.isNotEmpty) return DynamicLibrary.open(explicitPath);
  if (Platform.isIOS) return DynamicLibrary.process();
  if (Platform.isAndroid) return DynamicLibrary.open('libmobile_core.so');
  if (Platform.isMacOS) return DynamicLibrary.open('libmobile_core.dylib');
  if (Platform.isWindows) return DynamicLibrary.open('mobile_core.dll');
  return DynamicLibrary.open('libmobile_core.so');
}

typedef _EventCallbackNative = Void Function(Pointer<Utf8>);
typedef _NewNative = Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _NewDart = Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _FreeAppNative = Void Function(Pointer<Void>);
typedef _FreeAppDart = void Function(Pointer<Void>);
typedef _FreeStringNative = Void Function(Pointer<Utf8>);
typedef _FreeStringDart = void Function(Pointer<Utf8>);
typedef _JsonOperationNative = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>);
typedef _JsonOperationDart = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>);
typedef _HandleJsonNative = Pointer<Utf8> Function(Pointer<Void>);
typedef _HandleJsonDart = Pointer<Utf8> Function(Pointer<Void>);
typedef _TriggerSyncNative = Int32 Function(Pointer<Void>);
typedef _TriggerSyncDart = int Function(Pointer<Void>);
typedef _ResolveConflictNative = Int32 Function(Pointer<Void>, Pointer<Utf8>, Pointer<Utf8>);
typedef _ResolveConflictDart = int Function(Pointer<Void>, Pointer<Utf8>, Pointer<Utf8>);
typedef _SubscribeNative = Pointer<Void> Function(
  Pointer<Void>,
  Pointer<Utf8>,
  Pointer<NativeFunction<_EventCallbackNative>>,
);
typedef _SubscribeDart = Pointer<Void> Function(
  Pointer<Void>,
  Pointer<Utf8>,
  Pointer<NativeFunction<_EventCallbackNative>>,
);
typedef _UnsubscribeNative = Void Function(Pointer<Void>);
typedef _UnsubscribeDart = void Function(Pointer<Void>);
typedef _SetHierarchyNative = Int32 Function(Pointer<Void>, Pointer<Utf8>);
typedef _SetHierarchyDart = int Function(Pointer<Void>, Pointer<Utf8>);
typedef _UploadFileNative = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Utf8>,
);
typedef _UploadFileDart = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Utf8>,
);
typedef _DownloadFileNative = Int64 Function(Pointer<Void>, Pointer<Utf8>, Pointer<Utf8>);
typedef _DownloadFileDart = int Function(Pointer<Void>, Pointer<Utf8>, Pointer<Utf8>);

class _MobileCoreBindings {
  _MobileCoreBindings(DynamicLibrary library)
      : newApp = library.lookupFunction<_NewNative, _NewDart>('mobile_core_new'),
        freeApp = library.lookupFunction<_FreeAppNative, _FreeAppDart>('mobile_core_free'),
        freeString = library.lookupFunction<_FreeStringNative, _FreeStringDart>('mobile_core_free_string'),
        executeCommand = library.lookupFunction<_JsonOperationNative, _JsonOperationDart>(
          'mobile_core_execute_command',
        ),
        executeQuery = library.lookupFunction<_JsonOperationNative, _JsonOperationDart>(
          'mobile_core_execute_query',
        ),
        listAggregates = library.lookupFunction<_JsonOperationNative, _JsonOperationDart>(
          'mobile_core_list_aggregates',
        ),
        getSyncStatus = library.lookupFunction<_HandleJsonNative, _HandleJsonDart>(
          'mobile_core_get_sync_status',
        ),
        listConflicts = library.lookupFunction<_HandleJsonNative, _HandleJsonDart>(
          'mobile_core_list_conflicts',
        ),
        resolveConflict = library.lookupFunction<_ResolveConflictNative, _ResolveConflictDart>(
          'mobile_core_resolve_conflict',
        ),
        triggerSync = library.lookupFunction<_TriggerSyncNative, _TriggerSyncDart>(
          'mobile_core_trigger_sync',
        ),
        subscribeEvents = library.lookupFunction<_SubscribeNative, _SubscribeDart>(
          'mobile_core_subscribe_events',
        ),
        unsubscribe = library.lookupFunction<_UnsubscribeNative, _UnsubscribeDart>(
          'mobile_core_unsubscribe',
        ),
        setHierarchy = library.lookupFunction<_SetHierarchyNative, _SetHierarchyDart>(
          'mobile_core_set_hierarchy',
        ),
        uploadFile = library.lookupFunction<_UploadFileNative, _UploadFileDart>(
          'mobile_core_upload_file',
        ),
        downloadFile = library.lookupFunction<_DownloadFileNative, _DownloadFileDart>(
          'mobile_core_download_file',
        );

  final _NewDart newApp;
  final _FreeAppDart freeApp;
  final _FreeStringDart freeString;
  final _JsonOperationDart executeCommand;
  final _JsonOperationDart executeQuery;
  final _JsonOperationDart listAggregates;
  final _HandleJsonDart getSyncStatus;
  final _HandleJsonDart listConflicts;
  final _ResolveConflictDart resolveConflict;
  final _TriggerSyncDart triggerSync;
  final _SubscribeDart subscribeEvents;
  final _UnsubscribeDart unsubscribe;
  final _SetHierarchyDart setHierarchy;
  final _UploadFileDart uploadFile;
  final _DownloadFileDart downloadFile;
}

List<int> uuidToBytes(String uuid) {
  final normalized = uuid.replaceAll('-', '');
  if (!RegExp(r'^[0-9a-fA-F]{32}$').hasMatch(normalized)) {
    throw FormatException('Invalid UUID: $uuid');
  }
  return List<int>.generate(
    16,
    (int index) => int.parse(normalized.substring(index * 2, index * 2 + 2), radix: 16),
    growable: false,
  );
}

String bytesToUuid(List<int> bytes) {
  if (bytes.length != 16) return bytes.join('.');
  final hex = bytes.map((int value) => value.toRadixString(16).padLeft(2, '0')).join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-${hex.substring(12, 16)}-'
      '${hex.substring(16, 20)}-${hex.substring(20)}';
}

String randomUuid() {
  final random = Random.secure();
  final bytes = List<int>.generate(16, (_) => random.nextInt(256));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  return bytesToUuid(bytes);
}

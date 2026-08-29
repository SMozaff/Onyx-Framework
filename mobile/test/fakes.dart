import 'dart:async';

import 'package:onyx_mobile/bridge/bridge.dart';

class FakeOnyxApi implements OnyxApi {
  FakeOnyxApi({
    this.missions = const <LoadedAggregate>[],
    this.tasks = const <LoadedAggregate>[],
    this.conflicts = const <SyncConflict>[],
    this.snapshot = SyncSnapshot.empty,
    this.executeCommandOverride,
  });

  List<LoadedAggregate> missions;
  List<LoadedAggregate> tasks;
  List<SyncConflict> conflicts;
  SyncSnapshot snapshot;
  int syncCalls = 0;
  int commandCalls = 0;
  final controller = StreamController<Map<String, dynamic>>.broadcast();
  List<Map<String, dynamic>> executedEnvelopes = <Map<String, dynamic>>[];

  /// Lets a test replace the default always-succeeds behavior below —
  /// e.g. to simulate a real [CommandFailedException] denial the way
  /// `OnyxMobile.executeCommand` throws one for a real `{"success":
  /// false, "error": ...}` FFI payload, without needing a real native
  /// library loaded (this sandbox has no Android/iOS runtime to link
  /// one against).
  Future<Map<String, dynamic>> Function(Map<String, dynamic> envelope)? executeCommandOverride;

  @override
  Stream<Map<String, dynamic>> get events => controller.stream;

  /// Reuses the real [CommandEnvelopeFactory] rather than hand-rolling a
  /// second envelope shape here. Tests drive `OnyxController`'s create/
  /// update paths through [buildCommandEnvelope] and [encodeId], so a fake
  /// that built envelopes or encoded ids differently from `OnyxMobile`
  /// would let a genuine encoding regression pass. Ids match the defaults
  /// in main.dart.
  final envelopeFactory = CommandEnvelopeFactory(
    organizationId: '11111111-1111-1111-1111-111111111111',
    userId: '33333333-3333-4333-8333-333333333333',
  );

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
  }) =>
      envelopeFactory.create(
        commandType: commandType,
        targetType: targetType,
        targetId: targetId,
        payload: payload,
        expectedVersion: expectedVersion,
        lifecycleEpoch: lifecycleEpoch,
        authorityEpoch: authorityEpoch,
      );

  @override
  Future<Map<String, dynamic>> executeCommand(Map<String, dynamic> envelope) async {
    commandCalls += 1;
    executedEnvelopes.add(envelope);
    final override = executeCommandOverride;
    if (override != null) return override(envelope);
    return <String, dynamic>{'success': true, 'operation_id': envelope['operation_id']};
  }

  @override
  Future<dynamic> executeQuery(Map<String, dynamic> envelope) async => <String, dynamic>{'ok': true};

  @override
  Future<List<LoadedAggregate>> listAggregates(String aggregateType) async => switch (aggregateType) {
        'mission' => missions,
        'task' => tasks,
        _ => const <LoadedAggregate>[],
      };

  @override
  Future<SyncSnapshot> getSyncStatus() async => snapshot;

  @override
  Future<List<SyncConflict>> listConflicts() async => conflicts;

  @override
  Future<void> resolveConflict(SyncConflict conflict, ConflictChoice choice) async {
    conflicts = conflicts.where((item) => item != conflict).toList();
  }

  @override
  Future<void> triggerSync() async {
    syncCalls += 1;
  }

  @override
  Future<void> subscribeEvents({List<String>? eventTypes}) async {}

  @override
  Future<void> dispose() async => controller.close();

  int setHierarchyCalls = 0;

  @override
  Future<void> setHierarchy(String hierarchyJson) async {
    setHierarchyCalls += 1;
  }

  Map<String, dynamic>? lastUploadedPath;

  @override
  Future<Map<String, dynamic>> uploadFile(String path) async {
    lastUploadedPath = <String, dynamic>{'path': path};
    return <String, dynamic>{
      'file_asset_id': uuidToBytes('cccccccc-cccc-4ccc-8ccc-cccccccccccc'),
      'upload_session_id': uuidToBytes('dddddddd-dddd-4ddd-8ddd-dddddddddddd'),
      'content_hash': 'fake-content-hash',
      'size_bytes': 0,
    };
  }

  @override
  Future<int> downloadFile(String contentHash, String destinationPath) async => 0;
}

LoadedAggregate testMission({String name = 'Recovery Mission', String status = 'Active'}) => LoadedAggregate(
      rawId: uuidToBytes('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'),
      aggregate: <String, dynamic>{'name': name, 'status': status, 'description': 'Restore critical operations.'},
      version: 3,
      lifecycleEpoch: 1,
      authorityEpoch: 1,
      updatedAt: 1,
    );

LoadedAggregate testTask({String title = 'Verify backups', String status = 'Ready'}) => LoadedAggregate(
      rawId: uuidToBytes('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'),
      aggregate: <String, dynamic>{'title': title, 'status': status},
      version: 2,
      lifecycleEpoch: 0,
      authorityEpoch: 0,
      updatedAt: 1,
    );

SyncConflict testConflict() => SyncConflict.fromJson(<String, dynamic>{
      'conflict_id': List<int>.filled(16, 7),
      'field_path': 'status',
      'local_operation': <String, dynamic>{'payload': <String, dynamic>{'value': 'Active'}},
      'remote_operation': <String, dynamic>{'payload': <String, dynamic>{'value': 'Paused'}},
    });

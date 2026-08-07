import 'dart:async';

import 'package:onyx_mobile/bridge/bridge.dart';

class FakeOnyxApi implements OnyxApi {
  FakeOnyxApi({
    this.missions = const <LoadedAggregate>[],
    this.tasks = const <LoadedAggregate>[],
    this.conflicts = const <SyncConflict>[],
    this.snapshot = SyncSnapshot.empty,
  });

  List<LoadedAggregate> missions;
  List<LoadedAggregate> tasks;
  List<SyncConflict> conflicts;
  SyncSnapshot snapshot;
  int syncCalls = 0;
  int commandCalls = 0;
  final controller = StreamController<Map<String, dynamic>>.broadcast();

  @override
  Stream<Map<String, dynamic>> get events => controller.stream;

  @override
  Future<Map<String, dynamic>> executeCommand(Map<String, dynamic> envelope) async {
    commandCalls += 1;
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

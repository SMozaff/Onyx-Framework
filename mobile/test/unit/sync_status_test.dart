import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:onyx_mobile/bridge/bridge.dart';
import 'package:onyx_mobile/ui/widgets/sync_status.dart';

void main() {
  testWidgets('sync badges display offline, online, queued, and conflict states', (tester) async {
    Future<void> pump(SyncSnapshot snapshot, {bool network = true}) => tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: SyncStatusWidget(snapshot: snapshot, syncing: false, hasNetwork: network, onPressed: () {}),
            ),
          ),
        );

    await pump(SyncSnapshot.empty, network: false);
    expect(find.text('Offline'), findsOneWidget);
    await pump(const SyncSnapshot(online: true, pendingOutboxCount: 0, openConflictCount: 0));
    expect(find.text('Online'), findsOneWidget);
    await pump(const SyncSnapshot(online: false, pendingOutboxCount: 3, openConflictCount: 0));
    expect(find.text('Queued 3'), findsOneWidget);
    await pump(const SyncSnapshot(online: false, pendingOutboxCount: 0, openConflictCount: 1));
    expect(find.text('Conflict'), findsOneWidget);
  });
}

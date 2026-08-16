import 'package:flutter_test/flutter_test.dart';
import 'package:onyx_mobile/ui/app.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../fakes.dart';

void main() {
  testWidgets('dashboard renders stats, alerts, and mission list', (tester) async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    final preferences = await SharedPreferences.getInstance();
    final api = FakeOnyxApi(
      missions: [testMission()],
      tasks: [testTask()],
      conflicts: [testConflict()],
    );
    final controller = OnyxController(
      api: api,
      preferences: preferences,
      organizationId: '11111111-1111-1111-1111-111111111111',
      userId: '33333333-3333-4333-8333-333333333333',
      relayEndpoint: 'wss://relay.test',
    );
    await controller.refresh();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: controller, child: const OnyxApp()),
    );
    await tester.pumpAndSettle();
    expect(find.text('Command center'), findsOneWidget);
    expect(find.text('Recovery Mission'), findsWidgets);
    expect(find.textContaining('synchronization conflict'), findsOneWidget);
  });
}

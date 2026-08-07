import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:onyx_mobile/ui/app.dart';
import 'package:onyx_mobile/ui/widgets/conflict_dialog.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../fakes.dart';

void main() {
  testWidgets('conflict dialog compares values and accepts a resolution', (tester) async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    final preferences = await SharedPreferences.getInstance();
    final conflict = testConflict();
    final api = FakeOnyxApi(conflicts: <dynamic>[conflict].cast());
    final controller = OnyxController(
      api: api,
      preferences: preferences,
      organizationId: '11111111-1111-1111-1111-111111111111',
      userId: '33333333-3333-4333-8333-333333333333',
      relayEndpoint: 'wss://relay.test',
    );
    await controller.refresh();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: MaterialApp(home: Scaffold(body: ConflictDialog(conflict: conflict))),
      ),
    );
    expect(find.text('Active'), findsOneWidget);
    expect(find.text('Paused'), findsOneWidget);
    await tester.tap(find.text('Accept local'));
    await tester.pumpAndSettle();
    expect(api.conflicts, isEmpty);
  });
}

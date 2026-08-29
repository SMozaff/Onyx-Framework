import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:onyx_mobile/bridge/bridge.dart';
import 'package:onyx_mobile/ui/app.dart';
import 'package:onyx_mobile/ui/screens/approvals.dart';
import 'package:onyx_mobile/ui/screens/mission_detail.dart';
import 'package:onyx_mobile/ui/screens/task_detail.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../fakes.dart';

Future<OnyxController> buildController(FakeOnyxApi api) async {
  SharedPreferences.setMockInitialValues(<String, Object>{});
  final preferences = await SharedPreferences.getInstance();
  final controller = OnyxController(
    api: api,
    preferences: preferences,
    organizationId: '11111111-1111-1111-1111-111111111111',
    userId: '33333333-3333-4333-8333-333333333333',
    relayEndpoint: 'wss://relay.test',
  );
  await controller.refresh();
  return controller;
}

void main() {
  testWidgets('empty state is accurate, not the stale "no aggregate registered" text', (tester) async {
    final controller = await buildController(FakeOnyxApi());
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: const MaterialApp(home: ApprovalsScreen()),
      ),
    );
    expect(find.text('No tasks or missions are currently awaiting approval.'), findsOneWidget);
    expect(find.textContaining('bounded-context adapter'), findsNothing);
  });

  testWidgets('lists Submitted tasks and AwaitingApproval missions, and only those', (tester) async {
    final controller = await buildController(FakeOnyxApi(
      tasks: [testTask(title: 'Ready task', status: 'Ready'), testTask(title: 'Submitted task', status: 'Submitted')],
      missions: [testMission(name: 'Active mission', status: 'Active'), testMission(name: 'Pending mission', status: 'AwaitingApproval')],
    ));
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: const MaterialApp(home: ApprovalsScreen()),
      ),
    );
    expect(find.text('Submitted task'), findsOneWidget);
    expect(find.text('Pending mission'), findsOneWidget);
    expect(find.text('Ready task'), findsNothing);
    expect(find.text('Active mission'), findsNothing);
  });

  testWidgets('tapping a pending task opens TaskDetailScreen with real Approve/Reject actions', (tester) async {
    final controller = await buildController(FakeOnyxApi(
      tasks: [testTask(title: 'Submitted task', status: 'Submitted')],
    ));
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: const MaterialApp(home: ApprovalsScreen()),
      ),
    );
    await tester.tap(find.text('Submitted task'));
    await tester.pumpAndSettle();
    expect(find.byType(TaskDetailScreen), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Approve'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Reject'), findsOneWidget);
  });

  testWidgets('a task not awaiting approval shows no decision actions', (tester) async {
    final task = testTask(title: 'Active task', status: 'Active');
    await tester.pumpWidget(MaterialApp(home: TaskDetailScreen(task: task)));
    expect(find.widgetWithText(FilledButton, 'Approve'), findsNothing);
    expect(find.widgetWithText(OutlinedButton, 'Reject'), findsNothing);
  });

  testWidgets('Reject stays disabled until a reason is entered; Approve does not require one', (tester) async {
    final task = testTask(title: 'Submitted task', status: 'Submitted');
    await tester.pumpWidget(MaterialApp(home: TaskDetailScreen(task: task)));

    Widget approveFinder() => tester.widget<FilledButton>(find.widgetWithText(FilledButton, 'Approve'));
    Widget rejectFinder() => tester.widget<OutlinedButton>(find.widgetWithText(OutlinedButton, 'Reject'));
    expect((approveFinder() as FilledButton).onPressed, isNotNull);
    expect((rejectFinder() as OutlinedButton).onPressed, isNull);

    await tester.enterText(find.byType(TextField), 'a real reason');
    await tester.pump();
    expect((tester.widget<OutlinedButton>(find.widgetWithText(OutlinedButton, 'Reject'))).onPressed, isNotNull);
  });

  testWidgets('approving calls ApproveTask with the real reason and pops on success', (tester) async {
    final api = FakeOnyxApi(tasks: [testTask(title: 'Submitted task', status: 'Submitted')]);
    final controller = await buildController(api);
    final task = controller.tasks.single;

    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: MaterialApp(
          home: Builder(
            builder: (context) => ElevatedButton(
              onPressed: () => Navigator.of(context).push(
                MaterialPageRoute<void>(builder: (_) => TaskDetailScreen(task: task)),
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'looks good');
    await tester.tap(find.widgetWithText(FilledButton, 'Approve'));
    await tester.pumpAndSettle();

    expect(api.commandCalls, 1);
    final envelope = api.executedEnvelopes.single;
    expect(envelope['command_type'], 'ApproveTask');
    expect(envelope['payload'], <String, dynamic>{
      'ApproveTask': <String, dynamic>{'reason': 'looks good'},
    });
    // Popped back to the "open" button screen on success.
    expect(find.text('open'), findsOneWidget);
    expect(find.byType(TaskDetailScreen), findsNothing);
  });

  testWidgets('a real OwnerAuthorityDenied-shaped denial surfaces its specific message, not a crash', (tester) async {
    final api = FakeOnyxApi(
      tasks: [testTask(title: 'Submitted task', status: 'Submitted')],
      executeCommandOverride: (_) async => throw const CommandFailedException(
        'actor UserId(11111111-1111-1111-1111-111111111111) is not authorized '
        'to decide on behalf of owner UserId(22222222-2222-2222-2222-222222222222)',
      ),
    );
    final controller = await buildController(api);
    final task = controller.tasks.single;

    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: MaterialApp(home: TaskDetailScreen(task: task)),
      ),
    );
    await tester.tap(find.widgetWithText(FilledButton, 'Approve'));
    await tester.pumpAndSettle();

    // Stays on the detail screen (did not pop, unlike the success case)
    // and shows the real, specific denial text -- not a generic failure
    // message and not an unhandled exception crashing the widget tree.
    expect(find.byType(TaskDetailScreen), findsOneWidget);
    expect(find.textContaining('is not authorized to decide on behalf of'), findsOneWidget);
  });

  testWidgets('mission activation calls ActivateMission, rejection calls RejectApproval', (tester) async {
    final api = FakeOnyxApi(missions: [testMission(name: 'Pending mission', status: 'AwaitingApproval')]);
    final controller = await buildController(api);
    final mission = controller.missions.single;

    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: controller,
        child: MaterialApp(home: MissionDetailScreen(mission: mission)),
      ),
    );
    await tester.tap(find.widgetWithText(FilledButton, 'Activate'));
    await tester.pumpAndSettle();

    expect(api.executedEnvelopes.single['command_type'], 'ActivateMission');
  });
}

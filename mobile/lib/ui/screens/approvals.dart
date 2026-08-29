import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import 'mission_detail.dart';
import 'task_detail.dart';

/// Real pending-approval queue: Tasks in `Submitted` and Missions in
/// `AwaitingApproval`, filtered from `controller.tasks`/`controller.
/// missions` — the same local projections `TasksScreen`/`MissionsScreen`
/// already load via `listAggregates('task'/'mission')`, not a separate
/// server-side "list everything pending" query.
///
/// This deliberately does NOT call `listAggregates('approval')` (what
/// `controller.approvals` is backed by) — confirmed directly that no
/// aggregate is ever stored under that type in this app's local
/// database: `client-composition::app_state`'s `AppStateConfig`
/// registers repositories for mission/task/conversation/message/
/// file_asset/upload_session/policy/legal_hold/connection_request/
/// notification, never "approval". A separate, generic `ApprovalAggregate`
/// does exist server-side (`api-server::routes::command`, `"approval.
/// Approve"`/`"approval.Reject"`), but it is a different, unrelated
/// concept with no owner-authority gate at all, never wired into
/// `client-composition` (so never reachable from mobile/desktop's local
/// command path either) — not what this screen is for. The stale
/// placeholder text this replaced ("no local Approval aggregate is
/// registered... ready for the bounded-context adapter when it is
/// delivered") was literally accurate about that separate, undelivered
/// concept, but that was never what actually gates Task/Mission
/// approval — `ApproveTask`/`RejectTask`/`RejectApproval`/
/// `ActivateMission` are, and those operate on the Task/Mission
/// aggregates this screen now reads.
class ApprovalsScreen extends StatelessWidget {
  const ApprovalsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    final pendingTasks = controller.tasks.where((task) => task.status == 'Submitted').toList(growable: false);
    final pendingMissions = controller.missions.where((mission) => mission.status == 'AwaitingApproval').toList(growable: false);
    final isEmpty = pendingTasks.isEmpty && pendingMissions.isEmpty;

    return RefreshIndicator(
      onRefresh: controller.refresh,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Text('Approvals', style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 4),
          const Text('Tasks and missions awaiting your decision.'),
          const SizedBox(height: 16),
          if (isEmpty)
            const Card(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text('No tasks or missions are currently awaiting approval.'),
              ),
            )
          else ...<Widget>[
            for (final task in pendingTasks)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Card(
                  child: ListTile(
                    leading: const Icon(Icons.task_alt_outlined),
                    title: Text(task.title),
                    subtitle: Text(task.description ?? 'Task ${task.id}'),
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute<void>(builder: (_) => TaskDetailScreen(task: task)),
                    ),
                  ),
                ),
              ),
            for (final mission in pendingMissions)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Card(
                  child: ListTile(
                    leading: const Icon(Icons.flag_outlined),
                    title: Text(mission.title),
                    subtitle: Text(mission.description ?? 'Mission ${mission.id}'),
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute<void>(builder: (_) => MissionDetailScreen(mission: mission)),
                    ),
                  ),
                ),
              ),
          ],
        ],
      ),
    );
  }
}

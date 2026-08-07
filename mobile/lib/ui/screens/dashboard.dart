import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import '../widgets/mission_card.dart';

class DashboardScreen extends StatelessWidget {
  const DashboardScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return RefreshIndicator(
      onRefresh: controller.refresh,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Text('Command center', style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 16),
          GridView.count(
            crossAxisCount: MediaQuery.sizeOf(context).width > 600 ? 4 : 2,
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            crossAxisSpacing: 12,
            mainAxisSpacing: 12,
            childAspectRatio: 1.8,
            children: <Widget>[
              _StatCard(label: 'Missions', value: controller.missions.length, icon: Icons.flag),
              _StatCard(label: 'Tasks', value: controller.tasks.length, icon: Icons.task_alt),
              _StatCard(label: 'Conflicts', value: controller.conflicts.length, icon: Icons.warning_amber),
              _StatCard(label: 'Queued', value: controller.sync.pendingOutboxCount, icon: Icons.schedule),
            ],
          ),
          if (controller.error case final error?) ...<Widget>[
            const SizedBox(height: 16),
            Card(
              child: ListTile(
                leading: const Icon(Icons.error_outline),
                title: const Text('Local core unavailable'),
                subtitle: Text(error.toString()),
                trailing: IconButton(onPressed: controller.refresh, icon: const Icon(Icons.refresh)),
              ),
            ),
          ],
          if (controller.conflicts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 16),
            Card(
              child: ListTile(
                leading: const Icon(Icons.warning_amber_rounded, color: Color(0xFFC52A2A)),
                title: const Text('Conflict review required'),
                subtitle: Text('${controller.conflicts.length} authority-sensitive conflict(s) are open.'),
              ),
            ),
          ],
          const SizedBox(height: 24),
          Row(
            children: <Widget>[
              Expanded(child: Text('Active missions', style: Theme.of(context).textTheme.titleLarge)),
              TextButton(
                onPressed: () => controller.selectNavigation(1),
                child: const Text('View all'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          if (controller.missions.isEmpty)
            const _EmptyMessage(
              icon: Icons.flag_outlined,
              title: 'No missions yet',
              message: 'Create the first mission from the Missions tab.',
            )
          else
            ...controller.missions.take(3).map((mission) => Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: MissionCard(mission: mission),
                )),
          const SizedBox(height: 24),
          Text('Recent activity', style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 12),
          Card(
            child: controller.missions.isEmpty && controller.tasks.isEmpty
                ? const Padding(
                    padding: EdgeInsets.all(20),
                    child: Text('Domain events will appear here as local commands are accepted.'),
                  )
                : Column(
                    children: <Widget>[
                      ...controller.missions.take(2).map((mission) => ListTile(
                            leading: const Icon(Icons.flag_outlined),
                            title: Text(mission.title),
                            subtitle: Text('Mission ${mission.status} · version ${mission.version}'),
                          )),
                      ...controller.tasks.take(2).map((task) => ListTile(
                            leading: const Icon(Icons.task_alt_outlined),
                            title: Text(task.title),
                            subtitle: Text('Task ${task.status} · version ${task.version}'),
                          )),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({required this.label, required this.value, required this.icon});
  final String label;
  final num value;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Row(
          children: <Widget>[
            Icon(icon, color: Theme.of(context).colorScheme.primary),
            const SizedBox(width: 12),
            Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text('$value', style: Theme.of(context).textTheme.headlineSmall),
                Text(label, style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyMessage extends StatelessWidget {
  const _EmptyMessage({required this.icon, required this.title, required this.message});
  final IconData icon;
  final String title;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(28),
        child: Column(
          children: <Widget>[
            Icon(icon, size: 40),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(message, textAlign: TextAlign.center),
          ],
        ),
      ),
    );
  }
}

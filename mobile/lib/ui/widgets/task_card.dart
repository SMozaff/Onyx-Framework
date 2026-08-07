import 'package:flutter/material.dart';

import '../../bridge/bridge.dart';
import '../screens/task_detail.dart';
import 'status_badge.dart';

class TaskCard extends StatelessWidget {
  const TaskCard({super.key, required this.task});

  final LoadedAggregate task;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        contentPadding: const EdgeInsets.all(16),
        title: Text(task.title),
        subtitle: Text('Version ${task.version}'),
        trailing: StatusBadge(status: task.status),
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute<void>(builder: (_) => TaskDetailScreen(task: task)),
        ),
      ),
    );
  }
}

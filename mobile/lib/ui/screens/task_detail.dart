import 'dart:convert';

import 'package:flutter/material.dart';

import '../../bridge/bridge.dart';
import '../widgets/status_badge.dart';

class TaskDetailScreen extends StatelessWidget {
  const TaskDetailScreen({super.key, required this.task});
  final LoadedAggregate task;

  @override
  Widget build(BuildContext context) {
    const encoder = JsonEncoder.withIndent('  ');
    return Scaffold(
      appBar: AppBar(title: Text(task.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Row(children: <Widget>[Expanded(child: Text(task.title, style: Theme.of(context).textTheme.headlineSmall)), StatusBadge(status: task.status)]),
          if (task.description case final description?) ...<Widget>[const SizedBox(height: 12), Text(description)],
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Execution state', style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 12),
                  Text('Version: ${task.version}'),
                  Text('Lifecycle epoch: ${task.lifecycleEpoch}'),
                  Text('Authority epoch: ${task.authorityEpoch}'),
                  Text('ID: ${task.id}'),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          ExpansionTile(
            title: const Text('Raw local projection'),
            children: <Widget>[Padding(padding: const EdgeInsets.all(16), child: SelectableText(encoder.convert(task.aggregate)))],
          ),
        ],
      ),
    );
  }
}

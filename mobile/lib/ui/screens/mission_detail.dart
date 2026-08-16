import 'dart:convert';

import 'package:flutter/material.dart';

import '../../bridge/bridge.dart';
import '../widgets/status_badge.dart';

class MissionDetailScreen extends StatelessWidget {
  const MissionDetailScreen({super.key, required this.mission});
  final LoadedAggregate mission;

  @override
  Widget build(BuildContext context) {
    const encoder = JsonEncoder.withIndent('  ');
    return Scaffold(
      appBar: AppBar(title: Text(mission.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Row(children: <Widget>[Expanded(child: Text(mission.title, style: Theme.of(context).textTheme.headlineSmall)), StatusBadge(status: mission.status)]),
          if (mission.description case final description?) ...<Widget>[const SizedBox(height: 12), Text(description)],
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Authority state', style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 12),
                  Text('Object version: ${mission.version}'),
                  Text('Lifecycle epoch: ${mission.lifecycleEpoch}'),
                  Text('Authority epoch: ${mission.authorityEpoch}'),
                  Text('ID: ${mission.id}'),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          ExpansionTile(
            title: const Text('Raw local projection'),
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.all(16),
                child: SelectableText(encoder.convert(mission.aggregate)),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

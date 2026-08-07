import 'package:flutter/material.dart';

import '../../bridge/bridge.dart';
import '../screens/mission_detail.dart';
import 'status_badge.dart';

class MissionCard extends StatelessWidget {
  const MissionCard({super.key, required this.mission});

  final LoadedAggregate mission;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute<void>(builder: (_) => MissionDetailScreen(mission: mission)),
        ),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Row(
                children: <Widget>[
                  Expanded(child: Text(mission.title, style: Theme.of(context).textTheme.titleMedium)),
                  StatusBadge(status: mission.status),
                ],
              ),
              if (mission.description case final description?) ...<Widget>[
                const SizedBox(height: 8),
                Text(description, maxLines: 2, overflow: TextOverflow.ellipsis),
              ],
              const SizedBox(height: 12),
              Text('Version ${mission.version} · ${mission.id}', style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ),
      ),
    );
  }
}

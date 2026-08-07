import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import '../widgets/mission_card.dart';

class MissionsScreen extends StatelessWidget {
  const MissionsScreen({super.key});

  Future<void> _createMission(BuildContext context) async {
    final name = TextEditingController();
    final description = TextEditingController();
    final submitted = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Create mission'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            TextField(controller: name, autofocus: true, decoration: const InputDecoration(labelText: 'Name')),
            TextField(controller: description, decoration: const InputDecoration(labelText: 'Description')),
          ],
        ),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(dialogContext, false), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.pop(dialogContext, true), child: const Text('Create')),
        ],
      ),
    );
    if (submitted == true && name.text.trim().isNotEmpty && context.mounted) {
      await context.read<OnyxController>().createMission(
            name.text.trim(),
            description.text.trim().isEmpty ? null : description.text.trim(),
          );
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return Scaffold(
      backgroundColor: Colors.transparent,
      floatingActionButton: FloatingActionButton.extended(
        heroTag: 'mission-create',
        onPressed: () => _createMission(context),
        icon: const Icon(Icons.add),
        label: const Text('Mission'),
      ),
      body: RefreshIndicator(
        onRefresh: controller.refresh,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 96),
          children: <Widget>[
            Text('Missions', style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 4),
            const Text('Authoritative local mission state. Changes are accepted by the Rust domain core.'),
            const SizedBox(height: 16),
            if (controller.missions.isEmpty)
              const Card(child: Padding(padding: EdgeInsets.all(24), child: Text('No missions are stored on this device.')))
            else
              ...controller.missions.map((mission) => Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: MissionCard(mission: mission),
                  )),
          ],
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../bridge/bridge.dart';
import '../app.dart';
import '../widgets/task_card.dart';

class TasksScreen extends StatelessWidget {
  const TasksScreen({super.key});

  Future<void> _createTask(BuildContext context) async {
    final controller = context.read<OnyxController>();
    if (controller.missions.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Create a mission before adding tasks.')));
      return;
    }
    LoadedAggregate selectedMission = controller.missions.first;
    final title = TextEditingController();
    final description = TextEditingController();
    final submitted = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setState) => AlertDialog(
          title: const Text('Create task'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              DropdownButtonFormField<LoadedAggregate>(
                initialValue: selectedMission,
                decoration: const InputDecoration(labelText: 'Mission'),
                items: controller.missions
                    .map((mission) => DropdownMenuItem(value: mission, child: Text(mission.title)))
                    .toList(),
                onChanged: (mission) => setState(() => selectedMission = mission ?? selectedMission),
              ),
              TextField(controller: title, decoration: const InputDecoration(labelText: 'Title')),
              TextField(controller: description, decoration: const InputDecoration(labelText: 'Description')),
            ],
          ),
          actions: <Widget>[
            TextButton(onPressed: () => Navigator.pop(dialogContext, false), child: const Text('Cancel')),
            FilledButton(onPressed: () => Navigator.pop(dialogContext, true), child: const Text('Create')),
          ],
        ),
      ),
    );
    if (submitted == true && title.text.trim().isNotEmpty && context.mounted) {
      await controller.createTask(
        mission: selectedMission,
        title: title.text.trim(),
        description: description.text.trim().isEmpty ? null : description.text.trim(),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return Scaffold(
      backgroundColor: Colors.transparent,
      floatingActionButton: FloatingActionButton.extended(
        heroTag: 'task-create',
        onPressed: () => _createTask(context),
        icon: const Icon(Icons.add_task),
        label: const Text('Task'),
      ),
      body: RefreshIndicator(
        onRefresh: controller.refresh,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 96),
          children: <Widget>[
            Text('Tasks', style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 16),
            if (controller.tasks.isEmpty)
              const Card(child: Padding(padding: EdgeInsets.all(24), child: Text('No tasks are stored on this device.')))
            else
              ...controller.tasks.map((task) => Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: TaskCard(task: task),
                  )),
          ],
        ),
      ),
    );
  }
}

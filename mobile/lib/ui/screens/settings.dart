import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController organization;
  late final TextEditingController user;
  late final TextEditingController relay;

  @override
  void initState() {
    super.initState();
    final controller = context.read<OnyxController>();
    organization = TextEditingController(text: controller.organizationId);
    user = TextEditingController(text: controller.userId);
    relay = TextEditingController(text: controller.relayEndpoint);
  }

  @override
  void dispose() {
    organization.dispose();
    user.dispose();
    relay.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return ListView(
      padding: const EdgeInsets.all(16),
      children: <Widget>[
        Text('Settings', style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 16),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              children: <Widget>[
                TextField(controller: organization, decoration: const InputDecoration(labelText: 'Organization UUID')),
                TextField(controller: user, decoration: const InputDecoration(labelText: 'User UUID')),
                TextField(controller: relay, decoration: const InputDecoration(labelText: 'Cloud relay endpoint')),
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: () async {
                      try {
                        await controller.saveSettings(
                          organization: organization.text.trim(),
                          user: user.text.trim(),
                          relay: relay.text.trim(),
                        );
                        if (context.mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(content: Text('Settings saved. Restart the app to recreate mobile-core with the new tenant configuration.')),
                          );
                        }
                      } on FormatException catch (error) {
                        if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(error.message)));
                      }
                    },
                    child: const Text('Save'),
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        Card(
          child: ListTile(
            leading: const Icon(Icons.storage_outlined),
            title: const Text('Local-first database'),
            subtitle: Text('${controller.missions.length} missions · ${controller.tasks.length} tasks · ${controller.sync.pendingOutboxCount} queued events'),
          ),
        ),
      ],
    );
  }
}

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../main.dart';
import '../../net/onyx_http_api.dart';
import '../../net/session_storage.dart';
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
  late String _transportMode;

  @override
  void initState() {
    super.initState();
    final controller = context.read<OnyxController>();
    organization = TextEditingController(text: controller.organizationId);
    user = TextEditingController(text: controller.userId);
    relay = TextEditingController(text: controller.relayEndpoint);
    _transportMode = controller.preferences.getString('transport_mode') ?? 'ffi';
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
        // Transport mode: local-first (FFI/mobile-core, offline-capable,
        // syncs via the not-yet-implemented Cloud Relay — see
        // DECISIONS.md) vs. LAN (direct HTTP to api-server, requires it
        // running and reachable, no offline queueing — see
        // net/onyx_http_api.dart's doc comment on why getSyncStatus/
        // listConflicts/etc. are no-ops in this mode). A separate card
        // from the tenant-config one below because switching modes is a
        // fundamentally different kind of change (which OnyxApi
        // implementation main.dart constructs) than editing which
        // organization/user this same implementation acts as.
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text('Connection mode', style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 4),
                Text(
                  'Local-first works offline and syncs automatically once Cloud '
                  'Relay support is available. LAN connects directly to a running '
                  'api-server on your network — no offline support, and you\'ll '
                  'sign in again each time the app restarts.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
                // RadioListTile's own groupValue/onChanged were deprecated
                // after Flutter 3.32; the selected value and the change
                // callback now live on a RadioGroup ancestor, and each tile
                // carries only its own `value`.
                RadioGroup<String>(
                  groupValue: _transportMode,
                  onChanged: (value) => setState(() => _transportMode = value!),
                  child: const Column(
                    children: <Widget>[
                      RadioListTile<String>(
                        contentPadding: EdgeInsets.zero,
                        title: Text('Local-first'),
                        value: 'ffi',
                      ),
                      RadioListTile<String>(
                        contentPadding: EdgeInsets.zero,
                        title: Text('LAN (connect to api-server)'),
                        value: 'http',
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: () async {
                      await controller.preferences.setString('transport_mode', _transportMode);
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text(
                              'Connection mode saved. Restart the app to apply it.',
                            ),
                          ),
                        );
                      }
                    },
                    child: const Text('Save mode'),
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        // Hidden when the app is currently running in HTTP mode: userId
        // there is server-derived from login (see HttpLoginScreen /
        // OnyxHttpApi.loggedInUserId), not user-editable, and "Cloud
        // relay endpoint" has no meaning at all for a direct HTTP
        // connection (see onyx_http_api.dart's doc comment) — showing
        // editable fields for values that either can't take effect or
        // don't apply would be actively misleading, not just unused.
        // Checked against the *active* controller.api's runtime type,
        // not the possibly-just-changed-but-not-yet-applied
        // `_transportMode` radio selection above, since that only takes
        // effect after the restart the snackbar above already asks for.
        if (controller.api is! OnyxHttpApi)
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
          )
        else
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Connected via LAN', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 4),
                  Text(
                    'Organization: ${controller.organizationId}\n'
                    'Signed in as: ${controller.userId}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Restart the app to sign in again — you\'ll be prompted for '
                    'server address, organization, and credentials fresh each time.',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
          ),
        const SizedBox(height: 16),
        Card(
          child: ListTile(
            leading: Icon(controller.api is OnyxHttpApi ? Icons.wifi : Icons.storage_outlined),
            title: Text(controller.api is OnyxHttpApi ? 'Live server data' : 'Local-first database'),
            subtitle: Text(
              controller.api is OnyxHttpApi
                  ? '${controller.missions.length} missions · ${controller.tasks.length} tasks'
                  : '${controller.missions.length} missions · ${controller.tasks.length} tasks · ${controller.sync.pendingOutboxCount} queued events',
            ),
          ),
        ),
        if (controller.api is! OnyxHttpApi) ...<Widget>[
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Account', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 4),
                  Text(
                    'Signing out clears your saved login on this device — local data '
                    'and sync history stay put, but you\'ll need to sign in again to '
                    'resume Task/Mission approvals and syncing under your account.',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 12),
                  Align(
                    alignment: Alignment.centerRight,
                    child: OutlinedButton(
                      onPressed: () async {
                        await FfiSessionStorage.clear();
                        await controller.preferences.remove('organization_id');
                        await controller.preferences.remove('user_id');
                        await controller.preferences.setBool(hasRealFfiSessionKey, false);
                        await controller.api.dispose();
                        await restartApp();
                      },
                      child: const Text('Sign out'),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ],
    );
  }
}

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import '../widgets/status_badge.dart';

class NotificationsScreen extends StatelessWidget {
  const NotificationsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return RefreshIndicator(
      onRefresh: controller.refresh,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Text('Notifications', style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 16),
          if (controller.notifications.isEmpty)
            const Card(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text('No local Notification aggregate is available yet. Remote notification delivery remains available through the web client.'),
              ),
            )
          else
            ...controller.notifications.map((notification) => Card(
                  child: ListTile(
                    leading: const Icon(Icons.notifications_outlined),
                    title: Text(notification.title),
                    subtitle: Text(notification.description ?? 'Notification ${notification.id}'),
                    trailing: StatusBadge(status: notification.status),
                  ),
                )),
        ],
      ),
    );
  }
}

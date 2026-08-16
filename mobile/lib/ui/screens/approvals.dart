import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import '../widgets/status_badge.dart';

class ApprovalsScreen extends StatelessWidget {
  const ApprovalsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return RefreshIndicator(
      onRefresh: controller.refresh,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Text('Approvals', style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 16),
          if (controller.approvals.isEmpty)
            const Card(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text('No local Approval aggregate is registered in mobile-core. The screen is ready for the bounded-context adapter when it is delivered.'),
              ),
            )
          else
            ...controller.approvals.map((approval) => Card(
                  child: ListTile(
                    leading: const Icon(Icons.approval_outlined),
                    title: Text(approval.title),
                    subtitle: Text(approval.description ?? 'Approval ${approval.id}'),
                    trailing: StatusBadge(status: approval.status),
                  ),
                )),
        ],
      ),
    );
  }
}

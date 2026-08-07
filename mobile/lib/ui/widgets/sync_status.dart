import 'package:flutter/material.dart';

import '../../bridge/bridge.dart';

class SyncStatusWidget extends StatelessWidget {
  const SyncStatusWidget({
    super.key,
    required this.snapshot,
    required this.syncing,
    required this.hasNetwork,
    required this.onPressed,
  });

  final SyncSnapshot snapshot;
  final bool syncing;
  final bool hasNetwork;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final (label, icon, color) = syncing
        ? ('Syncing', Icons.sync, const Color(0xFF1A8CBF))
        : snapshot.openConflictCount > 0
            ? ('Conflict', Icons.warning_amber, const Color(0xFFC52A2A))
            : snapshot.pendingOutboxCount > 0
                ? ('Queued ${snapshot.pendingOutboxCount}', Icons.schedule, const Color(0xFFD4A020))
                : snapshot.online
                    ? ('Online', Icons.cloud_done, const Color(0xFF1A8C38))
                    : hasNetwork
                        ? ('Local', Icons.devices, const Color(0xFF1A8CBF))
                        : ('Offline', Icons.cloud_off, const Color(0xFFC52A2A));
    return Semantics(
      button: true,
      label: 'Synchronization status: $label. Tap to synchronize now.',
      child: ActionChip(
        onPressed: syncing ? null : onPressed,
        avatar: syncing
            ? const SizedBox.square(dimension: 16, child: CircularProgressIndicator(strokeWidth: 2))
            : Icon(icon, size: 17, color: color),
        label: Text(label),
      ),
    );
  }
}

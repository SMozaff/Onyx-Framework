import 'package:flutter/material.dart';

class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final normalized = status.toLowerCase();
    final (color, icon) = switch (normalized) {
      'active' || 'approved' || 'closed' => (const Color(0xFF1A8C38), Icons.check_circle),
      'paused' || 'pending' || 'blocked' || 'submitted' => (const Color(0xFFD4A020), Icons.pause_circle),
      'halted' || 'cancelled' || 'rejected' => (const Color(0xFFC52A2A), Icons.error),
      _ => (const Color(0xFF1A8CBF), Icons.info),
    };
    return Semantics(
      label: 'Status: $status',
      child: Chip(
        visualDensity: VisualDensity.compact,
        avatar: Icon(icon, size: 16, color: color),
        label: Text(status),
        side: BorderSide(color: color.withValues(alpha: 0.4)),
      ),
    );
  }
}

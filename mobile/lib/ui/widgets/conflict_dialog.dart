import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../bridge/bridge.dart';
import '../app.dart';

class ConflictDialog extends StatefulWidget {
  const ConflictDialog({super.key, required this.conflict});

  final SyncConflict conflict;

  @override
  State<ConflictDialog> createState() => _ConflictDialogState();
}

class _ConflictDialogState extends State<ConflictDialog> {
  bool resolving = false;

  Future<void> resolve(ConflictChoice choice) async {
    setState(() => resolving = true);
    try {
      await context.read<OnyxController>().resolveConflict(widget.conflict, choice);
      if (mounted) Navigator.of(context).pop();
    } finally {
      if (mounted) setState(() => resolving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Conflict detected'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text('Field: ${widget.conflict.fieldPath}', style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 16),
              _ComparisonPanel(label: 'Local', value: widget.conflict.localValue),
              const SizedBox(height: 12),
              _ComparisonPanel(label: 'Remote', value: widget.conflict.remoteValue),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(onPressed: resolving ? null : () => resolve(ConflictChoice.escalate), child: const Text('Escalate')),
        OutlinedButton(onPressed: resolving ? null : () => resolve(ConflictChoice.remote), child: const Text('Accept remote')),
        FilledButton(onPressed: resolving ? null : () => resolve(ConflictChoice.local), child: const Text('Accept local')),
      ],
    );
  }
}

class _ComparisonPanel extends StatelessWidget {
  const _ComparisonPanel({required this.label, required this.value});

  final String label;
  final dynamic value;

  @override
  Widget build(BuildContext context) {
    const encoder = JsonEncoder.withIndent('  ');
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(label, style: Theme.of(context).textTheme.labelLarge),
            const SizedBox(height: 6),
            SelectableText(value is String ? value as String : encoder.convert(value)),
          ],
        ),
      ),
    );
  }
}

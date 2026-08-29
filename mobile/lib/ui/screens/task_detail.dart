import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../bridge/bridge.dart';
import '../app.dart';
import '../widgets/status_badge.dart';

class TaskDetailScreen extends StatefulWidget {
  const TaskDetailScreen({super.key, required this.task});
  final LoadedAggregate task;

  @override
  State<TaskDetailScreen> createState() => _TaskDetailScreenState();
}

class _TaskDetailScreenState extends State<TaskDetailScreen> {
  final _reasonController = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _reasonController.dispose();
    super.dispose();
  }

  /// `ApproveTask`/`RejectTask` are gated by the real owner-authority
  /// check (`TaskDecisionHandler`, `crates/domains/work-domain/src/
  /// command.rs`) — most actors legitimately cannot decide most tasks,
  /// so a denial here is an expected outcome, not a bug. The button is
  /// always shown rather than hidden in advance (matching
  /// `desktop-shell`'s `Approvals.tsx`/`ApprovalDialog`, which does the
  /// same): the backend fails closed either way, and mobile-core's FFI
  /// surface has no "can this actor decide" query to build a preemptive
  /// check on top of without inventing new FFI surface this task didn't
  /// ask for. A denial surfaces here as [CommandFailedException]'s own
  /// specific message (e.g. "actor ... is not authorized to decide on
  /// behalf of owner ...") since the FFI path now carries that through
  /// instead of collapsing every dispatch error to the same generic
  /// failure.
  Future<void> _decide(String commandType, String reason) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await context.read<OnyxController>().decide(
            target: widget.task,
            targetType: 'task',
            commandType: commandType,
            reason: reason,
          );
      if (mounted) Navigator.of(context).pop();
    } catch (error) {
      setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    const encoder = JsonEncoder.withIndent('  ');
    final task = widget.task;
    // Reason policy matches `desktop-shell`'s `ApprovalDialog` exactly,
    // for consistency across platforms: optional for Approve, required
    // for Reject (the Reject action stays disabled until non-empty).
    final canDecide = task.status == 'Submitted';
    return Scaffold(
      appBar: AppBar(title: Text(task.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Row(children: <Widget>[Expanded(child: Text(task.title, style: Theme.of(context).textTheme.headlineSmall)), StatusBadge(status: task.status)]),
          if (task.description case final description?) ...<Widget>[const SizedBox(height: 12), Text(description)],
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Execution state', style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 12),
                  Text('Version: ${task.version}'),
                  Text('Lifecycle epoch: ${task.lifecycleEpoch}'),
                  Text('Authority epoch: ${task.authorityEpoch}'),
                  Text('ID: ${task.id}'),
                ],
              ),
            ),
          ),
          if (canDecide) ...<Widget>[
            const SizedBox(height: 16),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text('Review submission', style: Theme.of(context).textTheme.titleMedium),
                    const SizedBox(height: 8),
                    TextField(
                      controller: _reasonController,
                      decoration: const InputDecoration(labelText: 'Reason (required to reject, optional to approve)'),
                      maxLines: 3,
                      onChanged: (_) => setState(() {}),
                    ),
                    const SizedBox(height: 12),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: <Widget>[
                        OutlinedButton(
                          onPressed: _busy || _reasonController.text.trim().isEmpty
                              ? null
                              : () => _decide('RejectTask', _reasonController.text.trim()),
                          child: const Text('Reject'),
                        ),
                        const SizedBox(width: 8),
                        FilledButton(
                          onPressed: _busy ? null : () => _decide('ApproveTask', _reasonController.text.trim()),
                          child: _busy
                              ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                              : const Text('Approve'),
                        ),
                      ],
                    ),
                    if (_error != null) ...<Widget>[
                      const SizedBox(height: 8),
                      Text(_error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
                    ],
                  ],
                ),
              ),
            ),
          ],
          const SizedBox(height: 16),
          ExpansionTile(
            title: const Text('Raw local projection'),
            children: <Widget>[Padding(padding: const EdgeInsets.all(16), child: SelectableText(encoder.convert(task.aggregate)))],
          ),
        ],
      ),
    );
  }
}

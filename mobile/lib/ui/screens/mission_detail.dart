import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../bridge/bridge.dart';
import '../app.dart';
import '../widgets/status_badge.dart';

class MissionDetailScreen extends StatefulWidget {
  const MissionDetailScreen({super.key, required this.mission});
  final LoadedAggregate mission;

  @override
  State<MissionDetailScreen> createState() => _MissionDetailScreenState();
}

class _MissionDetailScreenState extends State<MissionDetailScreen> {
  final _reasonController = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _reasonController.dispose();
    super.dispose();
  }

  /// `ActivateMission` (approve) / `RejectApproval` (reject) — Mission's
  /// own approval commands, NOT a mirror of Task's `ApproveTask`/
  /// `RejectTask` naming (`crates/domains/mission-domain/src/
  /// command.rs`). Same owner-authority gate
  /// (`MissionDecisionHandler`), same always-shown/fails-closed design
  /// as `TaskDetailScreen`'s identical action — see that screen's own
  /// doc comment for the full reasoning, which applies here unchanged.
  Future<void> _decide(String commandType, String reason) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await context.read<OnyxController>().decide(
            target: widget.mission,
            targetType: 'mission',
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
    final mission = widget.mission;
    // Reason policy matches `TaskDetailScreen`/`desktop-shell`'s
    // `ApprovalDialog`: optional for Approve (Activate), required for
    // Reject.
    final canDecide = mission.status == 'AwaitingApproval';
    return Scaffold(
      appBar: AppBar(title: Text(mission.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Row(children: <Widget>[Expanded(child: Text(mission.title, style: Theme.of(context).textTheme.headlineSmall)), StatusBadge(status: mission.status)]),
          if (mission.description case final description?) ...<Widget>[const SizedBox(height: 12), Text(description)],
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('Authority state', style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 12),
                  Text('Object version: ${mission.version}'),
                  Text('Lifecycle epoch: ${mission.lifecycleEpoch}'),
                  Text('Authority epoch: ${mission.authorityEpoch}'),
                  Text('ID: ${mission.id}'),
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
                    Text('Review approval request', style: Theme.of(context).textTheme.titleMedium),
                    const SizedBox(height: 8),
                    TextField(
                      controller: _reasonController,
                      decoration: const InputDecoration(labelText: 'Reason (required to reject, optional to activate)'),
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
                              : () => _decide('RejectApproval', _reasonController.text.trim()),
                          child: const Text('Reject'),
                        ),
                        const SizedBox(width: 8),
                        FilledButton(
                          onPressed: _busy ? null : () => _decide('ActivateMission', _reasonController.text.trim()),
                          child: _busy
                              ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                              : const Text('Activate'),
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
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.all(16),
                child: SelectableText(encoder.convert(mission.aggregate)),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

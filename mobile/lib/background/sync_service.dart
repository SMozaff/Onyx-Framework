import '../bridge/bridge.dart';

/// Mobile synchronization entry point. Transport discovery and selection
/// remain inside Rust; Flutter only initiates the operation and renders the
/// resulting status/conflict state.
class SyncService {
  const SyncService(this.api);

  final OnyxApi api;

  Future<void> startSync() => api.triggerSync();
}

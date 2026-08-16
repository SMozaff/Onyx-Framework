import '../bridge/bridge.dart' show randomUuid;
import 'http_client.dart';

/// Builds and sends command envelopes over HTTP, mirroring
/// `web-ui/src/api/command.ts`'s `buildCommandEnvelope`/`executeCommand`
/// field-for-field, and matching the server's `CommandRequest` struct
/// (`crates/bins/api-server/src/routes/mod.rs`) exactly — verified by
/// reading that struct directly, not inferred.
///
/// Deliberately NOT sharing `bridge/bridge.dart`'s `CommandEnvelopeFactory`:
/// that factory encodes `command_id`/`operation_id`/target `id`/
/// `organization_id` as raw byte arrays (`uuidToBytes(...)`), because it
/// feeds the local Rust FFI boundary, which expects bytes. `CommandRequest`
/// on the HTTP server side declares these as plain `String`s (matching
/// `crypto.randomUUID()`'s string output on the web-ui side) — reusing the
/// FFI factory unmodified here would silently send the wrong JSON shape.
/// UUIDs are generated with `bridge.dart`'s existing `randomUuid()` helper
/// (a correct, already-tested UUIDv4 implementation) rather than adding a
/// new `uuid` package dependency for something the codebase already has.
class OnyxHttpCommandApi {
  OnyxHttpCommandApi(this._client);

  final OnyxHttpClient _client;

  Map<String, dynamic> buildEnvelope({
    required String commandType,
    required String targetType,
    required String targetId,
    required String organizationId,
    required Map<String, dynamic> payload,
    int expectedVersion = 0,
    int expectedLifecycleEpoch = 0,
    int expectedAuthorityEpoch = 0,
  }) {
    return <String, dynamic>{
      'command_id': randomUuid(),
      'operation_id': randomUuid(),
      'command_type': commandType,
      'schema_version': '1.0',
      'target': <String, dynamic>{
        'id': targetId,
        'type': targetType,
        'organization_id': organizationId,
      },
      'expected_version': expectedVersion,
      'expected_lifecycle_epoch': expectedLifecycleEpoch,
      'expected_authority_epoch': expectedAuthorityEpoch,
      'issued_at': DateTime.now().toUtc().toIso8601String(),
      'vector_clock': <String, dynamic>{'entries': <String, int>{}},
      'correlation_id': randomUuid(),
      'causation_id': null,
      'payload': payload,
    };
  }

  /// Sends a pre-built envelope (from [buildEnvelope]) and returns the
  /// decoded `CommandResult` JSON (`success`, `operation_id`, `events`,
  /// optionally `result`/`error` — see `CommandRequest`'s sibling struct
  /// in `routes/mod.rs` for the exact response shape). Left as a raw
  /// `Map<String, dynamic>` rather than a typed model, matching how
  /// `bridge.dart`'s `OnyxApi.executeCommand` already returns
  /// `Map<String, dynamic>` for the FFI path — callers already handle
  /// this shape today.
  Future<Map<String, dynamic>> execute(Map<String, dynamic> envelope) async {
    final response = await _client.dio.post<Map<String, dynamic>>(
      '/api/command',
      data: envelope,
    );
    return response.data!;
  }
}

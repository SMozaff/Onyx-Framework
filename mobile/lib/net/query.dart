import 'dart:convert';

import '../bridge/bridge.dart' show randomUuid;
import 'http_client.dart';

/// Builds and sends query envelopes over HTTP, mirroring
/// `web-ui/src/api/query.ts`'s `buildQueryEnvelope`/`encodeEnvelope`/
/// `executeQuery`. The server's `/api/query` route takes the envelope as
/// a base64url-encoded JSON string in a query parameter (`Base64Envelope`
/// in `crates/bins/api-server/src/routes/query.rs`), not a JSON body —
/// this is GET, not POST, unlike `/api/command`. Confirmed by reading
/// that route directly.
class OnyxHttpQueryApi {
  OnyxHttpQueryApi(this._client);

  final OnyxHttpClient _client;

  Map<String, dynamic> buildEnvelope({
    required String queryType,
    required String organizationId,
    Map<String, dynamic> filters = const <String, dynamic>{},
    int limit = 100,
    String? cursor,
    String? sortBy,
    String sortOrder = 'desc',
  }) {
    return <String, dynamic>{
      'query_id': randomUuid(),
      'query_type': queryType,
      'schema_version': '1.0',
      'organization_id': organizationId,
      'filters': filters,
      'limit': limit,
      if (cursor != null) 'cursor': cursor,
      if (sortBy != null) 'sort_by': sortBy,
      'sort_order': sortOrder,
    };
  }

  /// base64url without padding, matching `query.ts`'s
  /// `.replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')` —
  /// Dart's `base64UrlEncode` already produces `-`/`_` in place of `+`/`/`,
  /// so only the trailing `=` padding needs stripping to match exactly
  /// (the server's `URL_SAFE_NO_PAD` decoder, confirmed in query.rs,
  /// requires no padding).
  String _encodeEnvelope(Map<String, dynamic> envelope) {
    final bytes = utf8.encode(jsonEncode(envelope));
    return base64Url.encode(bytes).replaceAll('=', '');
  }

  /// Returns the decoded `QueryResponse` JSON (`query_id`, `data`,
  /// `has_more`, `next_cursor`, optionally `total_count`, `freshness`).
  /// Left as raw `Map<String, dynamic>`/`dynamic`, matching how
  /// `bridge.dart`'s `OnyxApi.executeQuery` already returns `dynamic` for
  /// the FFI path.
  Future<dynamic> execute(Map<String, dynamic> envelope) async {
    final response = await _client.dio.get<Map<String, dynamic>>(
      '/api/query',
      queryParameters: <String, dynamic>{'envelope': _encodeEnvelope(envelope)},
    );
    return response.data;
  }
}

import 'dart:async';
import 'dart:convert';

import 'package:web_socket_channel/web_socket_channel.dart';

/// WebSocket client for `/api/events`, mirroring
/// `web-ui/src/api/events.ts`'s `EventStream` class: same reconnect
/// backoff schedule (starts at 1s, doubles, caps at 30s), same
/// `resync_required` message handling, same query-param bearer token
/// (`?token=...` — matches `routes/events.rs`'s `WebSocketAuth` struct,
/// which reads the token from the query string, not a header, since
/// browsers' native WebSocket API cannot set custom headers — the same
/// constraint web-ui works around this way, so this client matches it
/// for protocol compatibility even though Dart's WebSocket client could
/// technically set a header instead).
///
/// Kept as a broadcast [Stream] of decoded event maps, matching
/// `bridge.dart`'s `OnyxApi.events` getter shape (`Stream<Map<String,
/// dynamic>>`), so [OnyxHttpApi] can expose the same interface regardless
/// of which transport is active.
class OnyxHttpEventStream {
  OnyxHttpEventStream({
    required this.wsBaseUrl,
    required this.getToken,
  });

  final String wsBaseUrl;

  /// A callback rather than a fixed token string, so a reconnect after a
  /// token refresh (see docs/AUDIT_REGISTER.md H-02 on token lifetime)
  /// picks up the current token rather than retrying with a stale one.
  final String? Function() getToken;

  final StreamController<Map<String, dynamic>> _controller =
      StreamController<Map<String, dynamic>>.broadcast();
  final StreamController<String> _statusController = StreamController<String>.broadcast();

  WebSocketChannel? _channel;
  StreamSubscription<dynamic>? _channelSubscription;
  Duration _retryDelay = const Duration(seconds: 1);
  Timer? _reconnectTimer;
  bool _closedByUser = false;

  /// Decoded event envelopes. A `resync_required` control message from
  /// the server (see events.rs's Lagged-broadcast handling) is NOT
  /// forwarded here — see [onResyncRequired] instead, matching
  /// `events.ts`'s separate `onEvent`/`onResync` callbacks.
  Stream<Map<String, dynamic>> get events => _controller.stream;

  /// Fires once per server-signaled resync requirement (the connection
  /// fell behind and the client's local state may be stale — see
  /// `routes/events.rs`'s `Lagged` broadcast-error handling). Callers
  /// should treat this as "call the query API again to refresh", the
  /// same way `events.ts`'s `onResync` callback is used in web-ui.
  final StreamController<void> _resyncController = StreamController<void>.broadcast();
  Stream<void> get onResyncRequired => _resyncController.stream;

  /// 'connecting' | 'connected' | 'disconnected', matching events.ts's
  /// onStatus values exactly so any shared UI logic (e.g. sync_status.dart)
  /// could read either transport's status the same way in the future.
  Stream<String> get status => _statusController.stream;

  void connect({List<String>? eventTypes, List<String>? aggregateIds}) {
    _closedByUser = false;
    _connectInternal(eventTypes: eventTypes, aggregateIds: aggregateIds);
  }

  void _connectInternal({List<String>? eventTypes, List<String>? aggregateIds}) {
    _statusController.add('connecting');
    final token = getToken();
    if (token == null) {
      // No point opening a socket the server will just reject; treat as
      // disconnected and let the normal reconnect schedule retry once a
      // token becomes available (e.g. after login completes).
      _statusController.add('disconnected');
      _scheduleReconnect(eventTypes: eventTypes, aggregateIds: aggregateIds);
      return;
    }

    final uri = Uri.parse('$wsBaseUrl/api/events').replace(
      queryParameters: <String, String>{'token': token},
    );

    try {
      _channel = WebSocketChannel.connect(uri);
    } catch (error) {
      _statusController.add('disconnected');
      _scheduleReconnect(eventTypes: eventTypes, aggregateIds: aggregateIds);
      return;
    }

    _channelSubscription = _channel!.stream.listen(
      (dynamic message) {
        _retryDelay = const Duration(seconds: 1);
        _statusController.add('connected');
        try {
          final decoded = jsonDecode(message as String);
          if (decoded is Map && decoded.containsKey('event_id')) {
            _controller.add(Map<String, dynamic>.from(decoded));
          } else if (decoded is Map && decoded['type'] == 'resync_required') {
            _resyncController.add(null);
          }
        } catch (_) {
          // Malformed server message; ignored, matching events.ts's
          // identical silent-ignore for the same case — the query API
          // remains the authoritative source of truth regardless.
        }
      },
      onDone: () => _scheduleReconnect(eventTypes: eventTypes, aggregateIds: aggregateIds),
      onError: (Object _) => _channel?.sink.close(),
      cancelOnError: true,
    );

    // Server-side subscription filter, sent once the socket is open —
    // matches events.ts sending {type: 'subscribe', filter} on 'open'.
    // WebSocketChannel has no explicit 'open' event; the sink accepts
    // writes immediately and the underlying implementation queues them
    // until the handshake completes, so sending right after connect() is
    // equivalent in practice.
    final filter = <String, dynamic>{
      if (eventTypes != null && eventTypes.isNotEmpty) 'event_types': eventTypes,
      if (aggregateIds != null && aggregateIds.isNotEmpty) 'aggregate_ids': aggregateIds,
    };
    _channel!.sink.add(jsonEncode(<String, dynamic>{'type': 'subscribe', 'filter': filter}));
  }

  void _scheduleReconnect({List<String>? eventTypes, List<String>? aggregateIds}) {
    _statusController.add('disconnected');
    if (_closedByUser) return;
    final delay = _retryDelay;
    _retryDelay = _retryDelay * 2 > const Duration(seconds: 30)
        ? const Duration(seconds: 30)
        : _retryDelay * 2;
    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(
      delay,
      () => _connectInternal(eventTypes: eventTypes, aggregateIds: aggregateIds),
    );
  }

  void close() {
    _closedByUser = true;
    _reconnectTimer?.cancel();
    unawaited(_channelSubscription?.cancel());
    unawaited(_channel?.sink.close());
    _channel = null;
    _statusController.add('disconnected');
  }

  Future<void> dispose() async {
    close();
    await _controller.close();
    await _statusController.close();
    await _resyncController.close();
  }
}

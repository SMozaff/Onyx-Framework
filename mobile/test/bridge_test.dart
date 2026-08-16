import 'package:flutter_test/flutter_test.dart';
import 'package:onyx_mobile/bridge/bridge.dart';

import 'fakes.dart';

void main() {
  test('UUID conversion preserves the identifier', () {
    const uuid = '11111111-1111-4111-8111-111111111111';
    expect(bytesToUuid(uuidToBytes(uuid)), uuid);
  });

  test('command envelope contains authority and causality fields', () {
    final factory = CommandEnvelopeFactory(
      organizationId: '11111111-1111-1111-1111-111111111111',
      userId: '33333333-3333-4333-8333-333333333333',
    );
    final envelope = factory.create(
      commandType: 'CreateMission',
      targetType: 'mission',
      targetId: randomUuid(),
      payload: <String, dynamic>{'CreateMission': <String, dynamic>{'name': 'Test'}},
    );
    expect(envelope['authority_proof'], isA<Map<String, dynamic>>());
    expect(envelope['vector_clock'], <String, dynamic>{'entries': <String, int>{}});
    expect((envelope['operation_id'] as List<dynamic>).length, 16);
  });

  test('bridge abstraction executes commands and queries', () async {
    final api = FakeOnyxApi();
    expect(await api.executeQuery(<String, dynamic>{'query_type': 'GetMission'}), <String, dynamic>{'ok': true});
    final result = await api.executeCommand(<String, dynamic>{'operation_id': 'op'});
    expect(result['success'], isTrue);
    expect(api.commandCalls, 1);
  });
}

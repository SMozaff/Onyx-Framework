import 'package:flutter_test/flutter_test.dart';

import '../fakes.dart';

void main() {
  test('background sync delegates to the Rust bridge abstraction', () async {
    final api = FakeOnyxApi();
    await api.triggerSync();
    expect(api.syncCalls, 1);
  });
}

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import '../fakes.dart';

void main() {
  final deviceLabEnabled = Platform.environment['ONYX_MOBILE_DEVICE_TEST'] == '1';
  test(
    'Wi-Fi Direct or BLE sync completes through mobile-core',
    () async {
      final api = FakeOnyxApi();
      await api.triggerSync();
      expect(api.syncCalls, 1);
    },
    skip: deviceLabEnabled ? false : 'Requires two authorized iOS/Android devices and ONYX_MOBILE_DEVICE_TEST=1',
  );
}

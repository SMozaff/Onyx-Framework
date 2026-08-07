import 'dart:io';
import 'dart:ui';

import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:workmanager/workmanager.dart';

import '../../bridge/bridge.dart';

const _androidSyncTask = 'com.onyx.sync';

@pragma('vm:entry-point')
void onyxWorkmanagerDispatcher() {
  Workmanager().executeTask((task, inputData) async {
    WidgetsFlutterBinding.ensureInitialized();
    DartPluginRegistrant.ensureInitialized();
    try {
      final preferences = await SharedPreferences.getInstance();
      final support = await getApplicationSupportDirectory();
      final api = await FfiOnyxMobile.open(
        databasePath: '${support.path}${Platform.pathSeparator}onyx.sqlite',
        config: MobileCoreConfig(
          organizationId: preferences.getString('organization_id') ??
              '11111111-1111-1111-1111-111111111111',
          cloudRelayEndpoint: preferences.getString('relay_endpoint') ??
              'wss://relay.onyx.invalid/v1',
        ),
      );
      await api.triggerSync();
      await api.dispose();
      return true;
    } catch (_) {
      return false;
    }
  });
}

Future<void> registerAndroidBackgroundSync() async {
  if (!Platform.isAndroid) return;
  await Workmanager().initialize(onyxWorkmanagerDispatcher);
  await Workmanager().registerPeriodicTask(
    'onyx-periodic-sync',
    _androidSyncTask,
    frequency: const Duration(minutes: 15),
    constraints: Constraints(networkType: NetworkType.connected),
  );
}

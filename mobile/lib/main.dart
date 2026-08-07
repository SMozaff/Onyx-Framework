import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'background/android/workmanager_service.dart';
import 'background/ios/background_service.dart';
import 'bridge/bridge.dart';
import 'ui/app.dart';

const defaultOrganizationId = '11111111-1111-1111-1111-111111111111';
const defaultUserId = '33333333-3333-4333-8333-333333333333';
const defaultRelayEndpoint = 'wss://relay.onyx.invalid/v1';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final preferences = await SharedPreferences.getInstance();
  final supportDirectory = await getApplicationSupportDirectory();
  final organizationId = preferences.getString('organization_id') ?? defaultOrganizationId;
  final relayEndpoint = preferences.getString('relay_endpoint') ?? defaultRelayEndpoint;
  final api = await FfiOnyxMobile.open(
    databasePath: '${supportDirectory.path}${Platform.pathSeparator}onyx.sqlite',
    config: MobileCoreConfig(
      organizationId: organizationId,
      cloudRelayEndpoint: relayEndpoint,
    ),
  );
  await api.subscribeEvents();
  await registerAndroidBackgroundSync();
  await registerIosBackgroundSync();

  runApp(
    ChangeNotifierProvider<OnyxController>(
      create: (_) => OnyxController(
        api: api,
        preferences: preferences,
        organizationId: organizationId,
        userId: preferences.getString('user_id') ?? defaultUserId,
        relayEndpoint: relayEndpoint,
      )..initialize(),
      child: const OnyxApp(),
    ),
  );
}

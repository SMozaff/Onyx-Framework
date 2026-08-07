import 'dart:io';

import 'package:background_fetch/background_fetch.dart';

Future<void> registerIosBackgroundSync() async {
  if (!Platform.isIOS) return;
  await BackgroundFetch.configure(
    BackgroundFetchConfig(
      minimumFetchInterval: 15,
      stopOnTerminate: false,
      enableHeadless: true,
      requiredNetworkType: NetworkType.ANY,
    ),
    (taskId) async {
      // Native BackgroundService.swift calls the registered Rust core when
      // available. The plugin callback is retained to satisfy iOS fetch
      // completion semantics and permit future cursor-aware Dart work.
      BackgroundFetch.finish(taskId);
    },
    (taskId) async => BackgroundFetch.finish(taskId),
  );
}

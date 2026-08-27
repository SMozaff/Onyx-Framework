import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../bridge/bridge.dart';
import 'screens/approvals.dart';
import 'screens/dashboard.dart';
import 'screens/files.dart';
import 'screens/missions.dart';
import 'screens/notifications.dart';
import 'screens/settings.dart';
import 'screens/tasks.dart';
import 'widgets/conflict_dialog.dart';
import 'widgets/sync_status.dart';

class OnyxController extends ChangeNotifier {
  OnyxController({
    required this.api,
    required this.preferences,
    required this.organizationId,
    required this.userId,
    required this.relayEndpoint,
  });

  final OnyxApi api;
  final SharedPreferences preferences;
  String organizationId;
  String userId;
  String relayEndpoint;

  List<LoadedAggregate> missions = const <LoadedAggregate>[];
  List<LoadedAggregate> tasks = const <LoadedAggregate>[];
  List<LoadedAggregate> approvals = const <LoadedAggregate>[];
  List<LoadedAggregate> notifications = const <LoadedAggregate>[];
  List<SyncConflict> conflicts = const <SyncConflict>[];
  SyncSnapshot sync = SyncSnapshot.empty;
  bool isLoading = true;
  bool isSyncing = false;
  bool hasNetwork = true;
  Object? error;
  int navigationIndex = 0;
  StreamSubscription<dynamic>? _eventSubscription;
  StreamSubscription<List<ConnectivityResult>>? _connectivitySubscription;

  Future<void> initialize() async {
    _eventSubscription = api.events.listen((_) => refresh());
    _connectivitySubscription = Connectivity().onConnectivityChanged.listen((results) {
      hasNetwork = results.any((result) => result != ConnectivityResult.none);
      notifyListeners();
    });
    await refresh();
  }

  Future<void> refresh() async {
    try {
      error = null;
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        api.listAggregates('mission'),
        api.listAggregates('task'),
        api.listAggregates('approval'),
        api.listAggregates('notification'),
        api.getSyncStatus(),
        api.listConflicts(),
      ]);
      missions = results[0] as List<LoadedAggregate>;
      tasks = results[1] as List<LoadedAggregate>;
      approvals = results[2] as List<LoadedAggregate>;
      notifications = results[3] as List<LoadedAggregate>;
      sync = results[4] as SyncSnapshot;
      conflicts = results[5] as List<SyncConflict>;
    } catch (caught) {
      error = caught;
    } finally {
      isLoading = false;
      notifyListeners();
    }
  }

  Future<void> triggerSync() async {
    if (isSyncing) return;
    isSyncing = true;
    notifyListeners();
    try {
      await api.triggerSync();
      await refresh();
    } finally {
      isSyncing = false;
      notifyListeners();
    }
  }

  Future<void> createMission(String name, String? description) async {
    final missionId = randomUuid();
    await api.executeCommand(
      api.buildCommandEnvelope(
        commandType: 'CreateMission',
        targetType: 'mission',
        targetId: missionId,
        payload: <String, dynamic>{
          'CreateMission': <String, dynamic>{
            'name': name,
            'description': description,
            'owner_id': api.encodeId(userId),
          },
        },
      ),
    );
    await refresh();
  }

  Future<void> createTask({
    required LoadedAggregate mission,
    required String title,
    String? description,
  }) async {
    final taskId = randomUuid();
    await api.executeCommand(
      api.buildCommandEnvelope(
        commandType: 'CreateTask',
        targetType: 'task',
        targetId: taskId,
        payload: <String, dynamic>{
          'CreateTask': <String, dynamic>{
            'mission_id': api.encodeId(mission.id),
            'title': title,
            'description': description,
            'owner_id': api.encodeId(userId),
          },
        },
      ),
    );
    await refresh();
  }

  Future<void> resolveConflict(SyncConflict conflict, ConflictChoice choice) async {
    await api.resolveConflict(conflict, choice);
    await refresh();
  }

  /// Saves the Cloud Relay endpoint only. Deliberately does NOT accept
  /// `organization`/`user` overrides anymore — this method used to let
  /// anyone type in an arbitrary `organization_id`/`user_id` here and
  /// have mobile-core act as that identity on the next restart, with no
  /// connection to a real login at all. Now that FFI mode has a real
  /// login (`ui/ffi_login_screen.dart`), that was a real security hole,
  /// not a rough edge: it let a user bypass authentication entirely and
  /// silently defeated the whole point of real login/approval-authority
  /// gating. Identity now changes only via a real login or a
  /// secure-storage-backed sign-out (see `net/session_storage.dart`),
  /// never via free-text entry.
  Future<void> saveRelayEndpoint(String relay) async {
    relayEndpoint = relay;
    await preferences.setString('relay_endpoint', relay);
    notifyListeners();
  }

  void selectNavigation(int index) {
    navigationIndex = index;
    notifyListeners();
  }

  @override
  void dispose() {
    _eventSubscription?.cancel();
    _connectivitySubscription?.cancel();
    unawaited(api.dispose());
    super.dispose();
  }
}

class OnyxApp extends StatelessWidget {
  const OnyxApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'ONYX',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF3A7AB8),
          brightness: Brightness.light,
        ),
        scaffoldBackgroundColor: const Color(0xFFF5F7FA),
        cardTheme: const CardThemeData(
          elevation: 0,
          margin: EdgeInsets.zero,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(16)),
            side: BorderSide(color: Color(0xFFE1E6ED)),
          ),
        ),
      ),
      home: const _MobileShell(),
    );
  }
}

/// Wires an already-constructed [OnyxApi] into an [OnyxController] and
/// hosts [OnyxApp] beneath a [ChangeNotifierProvider], exactly the
/// `runApp(ChangeNotifierProvider<OnyxController>(...))` shape
/// `main.dart`'s `restartApp` previously built inline for the FFI path.
///
/// Factored out as its own widget (rather than kept inline in
/// `restartApp`) specifically so `HttpLoginScreen` can push it via
/// [Navigator] after a successful login — at that point the app is
/// already running inside a `MaterialApp` (the login screen's own), so a
/// second top-level `runApp()` call would be the wrong tool; `Navigator`
/// is. `restartApp` continues to use this same widget via `runApp()` for
/// the FFI path, so there is exactly one place that builds this wiring,
/// not two.
///
/// Deliberately takes already-resolved `organizationId`/`userId`/
/// `relayEndpoint` rather than reading `SharedPreferences` itself with
/// its own defaults: the FFI path's real values now come from a real
/// login (`ui/ffi_login_screen.dart`, persisted in `SharedPreferences`
/// only after that login succeeds — there is no placeholder default
/// left to fall back to) and HTTP-mode's `HttpLoginScreen` has
/// no equivalent defaults to fall back to (an empty LAN org ID is a
/// caller-visible bug, not a sensible default) — a single default set
/// baked into this widget would have been wrong for one path or the
/// other.
class OnyxControllerHost extends StatelessWidget {
  const OnyxControllerHost({
    super.key,
    required this.api,
    required this.preferences,
    required this.organizationId,
    required this.userId,
    required this.relayEndpoint,
  });

  final OnyxApi api;
  final SharedPreferences preferences;
  final String organizationId;
  final String userId;
  final String relayEndpoint;

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider<OnyxController>(
      create: (_) => OnyxController(
        api: api,
        preferences: preferences,
        organizationId: organizationId,
        userId: userId,
        relayEndpoint: relayEndpoint,
      )..initialize(),
      child: const OnyxApp(),
    );
  }
}

class _MobileShell extends StatelessWidget {
  const _MobileShell();

  static const pages = <Widget>[
    DashboardScreen(),
    MissionsScreen(),
    TasksScreen(),
    NotificationsScreen(),
    ApprovalsScreen(),
    FilesScreen(),
    SettingsScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return Scaffold(
      appBar: AppBar(
        title: const Text('ONYX'),
        actions: <Widget>[
          SyncStatusWidget(
            snapshot: controller.sync,
            syncing: controller.isSyncing,
            hasNetwork: controller.hasNetwork,
            onPressed: controller.triggerSync,
          ),
          const SizedBox(width: 12),
        ],
      ),
      body: Column(
        children: <Widget>[
          if (!controller.hasNetwork)
            const MaterialBanner(
              content: Text('Offline — commands remain local and will synchronize when a peer is available.'),
              leading: Icon(Icons.cloud_off),
              actions: <Widget>[SizedBox.shrink()],
            ),
          if (controller.conflicts.isNotEmpty)
            ListTile(
              tileColor: const Color(0xFFFCE5E5),
              leading: const Icon(Icons.warning_amber_rounded),
              title: Text('${controller.conflicts.length} synchronization conflict(s) require review'),
              trailing: const Icon(Icons.chevron_right),
              onTap: () => showDialog<void>(
                context: context,
                builder: (_) => ConflictDialog(conflict: controller.conflicts.first),
              ),
            ),
          Expanded(
            child: controller.isLoading
                ? const Center(child: CircularProgressIndicator())
                : IndexedStack(index: controller.navigationIndex, children: pages),
          ),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: controller.navigationIndex,
        onDestinationSelected: controller.selectNavigation,
        destinations: const <NavigationDestination>[
          NavigationDestination(icon: Icon(Icons.dashboard_outlined), selectedIcon: Icon(Icons.dashboard), label: 'Home'),
          NavigationDestination(icon: Icon(Icons.flag_outlined), selectedIcon: Icon(Icons.flag), label: 'Missions'),
          NavigationDestination(icon: Icon(Icons.task_alt_outlined), selectedIcon: Icon(Icons.task_alt), label: 'Tasks'),
          NavigationDestination(icon: Icon(Icons.notifications_outlined), selectedIcon: Icon(Icons.notifications), label: 'Alerts'),
          NavigationDestination(icon: Icon(Icons.approval_outlined), selectedIcon: Icon(Icons.approval), label: 'Approvals'),
          NavigationDestination(icon: Icon(Icons.folder_outlined), selectedIcon: Icon(Icons.folder), label: 'Files'),
          NavigationDestination(icon: Icon(Icons.settings_outlined), selectedIcon: Icon(Icons.settings), label: 'Settings'),
        ],
      ),
    );
  }
}

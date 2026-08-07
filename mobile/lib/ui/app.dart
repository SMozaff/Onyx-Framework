import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../bridge/bridge.dart';
import 'screens/approvals.dart';
import 'screens/dashboard.dart';
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
  }) : envelopeFactory = CommandEnvelopeFactory(
          organizationId: organizationId,
          userId: userId,
        );

  final OnyxApi api;
  final SharedPreferences preferences;
  final CommandEnvelopeFactory envelopeFactory;
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
      envelopeFactory.create(
        commandType: 'CreateMission',
        targetType: 'mission',
        targetId: missionId,
        payload: <String, dynamic>{
          'CreateMission': <String, dynamic>{
            'name': name,
            'description': description,
            'owner_id': uuidToBytes(userId),
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
      envelopeFactory.create(
        commandType: 'CreateTask',
        targetType: 'task',
        targetId: taskId,
        payload: <String, dynamic>{
          'CreateTask': <String, dynamic>{
            'mission_id': uuidToBytes(mission.id),
            'title': title,
            'description': description,
            'owner_id': uuidToBytes(userId),
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

  Future<void> saveSettings({
    required String organization,
    required String user,
    required String relay,
  }) async {
    uuidToBytes(organization);
    uuidToBytes(user);
    organizationId = organization;
    userId = user;
    relayEndpoint = relay;
    await preferences.setString('organization_id', organization);
    await preferences.setString('user_id', user);
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

class _MobileShell extends StatelessWidget {
  const _MobileShell();

  static const pages = <Widget>[
    DashboardScreen(),
    MissionsScreen(),
    TasksScreen(),
    NotificationsScreen(),
    ApprovalsScreen(),
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
          NavigationDestination(icon: Icon(Icons.settings_outlined), selectedIcon: Icon(Icons.settings), label: 'Settings'),
        ],
      ),
    );
  }
}

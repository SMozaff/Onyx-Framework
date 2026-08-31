package com.onyx.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.TaskAlt
import androidx.compose.material.icons.filled.WarningAmber
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.onyx.controller.OnyxController
import com.onyx.model.LoadedAggregate
import com.onyx.session.SessionPreferences
import com.onyx.ui.screens.DashboardScreen
import com.onyx.ui.screens.FilesScreen
import com.onyx.ui.screens.MissionDetailScreen
import com.onyx.ui.screens.MissionsScreen
import com.onyx.ui.screens.NotificationsScreen
import com.onyx.ui.screens.SettingsScreen
import com.onyx.ui.screens.TaskDetailScreen
import com.onyx.ui.screens.TasksScreen
import com.onyx.ui.widgets.ConflictDialog
import com.onyx.ui.widgets.SyncStatusIndicator

/**
 * The real post-login app shell, Kotlin's port of `ui/app.dart`'s
 * `_MobileShell`. A4 built the first four tabs (Home/Missions/Tasks/
 * Alerts); A5 adds Files and Settings, plus the top app bar's sync
 * status indicator and the conflict-review banner/dialog -- the same
 * six of Dart's seven real `NavigationDestination`s this project has
 * built so far. Approvals (Dart's 5th destination) remains
 * deliberately out of scope, not a silent gap -- a later task, per A5's
 * own "do not build beyond Files/Settings/sync/conflict/background
 * sync" instruction.
 *
 * Mission/Task Detail are pushed as a full-screen overlay on top of the
 * shell (mirroring `Navigator.push`), taking a [LoadedAggregate]
 * snapshot rather than watching [controller] live -- the one real
 * exception the parity matrix documents (§12): Detail screens freeze
 * their displayed version/status as of navigation time.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppShell(controller: OnyxController, onSignOut: () -> Unit) {
    var selectedTab by rememberSaveable { mutableStateOf(0) }
    var openMission by remember { mutableStateOf<LoadedAggregate?>(null) }
    var openTask by remember { mutableStateOf<LoadedAggregate?>(null) }
    var conflictDialogIndex by remember { mutableStateOf(-1) }
    val context = LocalContext.current

    val missions by controller.missions.collectAsState()
    val tasks by controller.tasks.collectAsState()
    val notifications by controller.notifications.collectAsState()
    val sync by controller.sync.collectAsState()
    val conflicts by controller.conflicts.collectAsState()
    val conflictCount by controller.conflictCount.collectAsState()
    val isSyncing by controller.isSyncing.collectAsState()
    val hasNetwork by controller.hasNetwork.collectAsState()
    val error by controller.error.collectAsState()

    if (openMission != null) {
        // Re-resolve from the live list on every recomposition so a
        // decide()-triggered refresh() that changes this mission's
        // status is reflected if the shell rebuilds while still open --
        // still a snapshot per individual Detail render (matches
        // Dart), just not permanently stale across a full shell rebuild.
        val current = missions.find { it.id == openMission!!.id } ?: openMission!!
        MissionDetailScreen(mission = current, controller = controller, onBack = { openMission = null })
        return
    }
    if (openTask != null) {
        val current = tasks.find { it.id == openTask!!.id } ?: openTask!!
        TaskDetailScreen(task = current, controller = controller, onBack = { openTask = null })
        return
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("ONYX") },
                actions = {
                    SyncStatusIndicator(
                        snapshot = sync,
                        syncing = isSyncing,
                        hasNetwork = hasNetwork,
                        onClick = { controller.triggerSync() },
                    )
                    androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(end = 12.dp))
                },
            )
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(selected = selectedTab == 0, onClick = { selectedTab = 0 }, icon = { Icon(Icons.Filled.Dashboard, contentDescription = null) }, label = { Text("Home") })
                NavigationBarItem(selected = selectedTab == 1, onClick = { selectedTab = 1 }, icon = { Icon(Icons.Filled.Flag, contentDescription = null) }, label = { Text("Missions") })
                NavigationBarItem(selected = selectedTab == 2, onClick = { selectedTab = 2 }, icon = { Icon(Icons.Filled.TaskAlt, contentDescription = null) }, label = { Text("Tasks") })
                NavigationBarItem(selected = selectedTab == 3, onClick = { selectedTab = 3 }, icon = { Icon(Icons.Filled.Notifications, contentDescription = null) }, label = { Text("Alerts") })
                NavigationBarItem(selected = selectedTab == 4, onClick = { selectedTab = 4 }, icon = { Icon(Icons.Filled.Folder, contentDescription = null) }, label = { Text("Files") })
                NavigationBarItem(selected = selectedTab == 5, onClick = { selectedTab = 5 }, icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Settings") })
            }
        },
    ) { padding ->
        androidx.compose.foundation.layout.Column(modifier = Modifier.padding(padding)) {
            if (conflicts.isNotEmpty()) {
                val current = conflicts.getOrNull(conflictDialogIndex.coerceIn(0, conflicts.size - 1))
                ListItem(
                    headlineContent = { Text("${conflicts.size} synchronization conflict(s) require review") },
                    leadingContent = { Icon(Icons.Filled.WarningAmber, contentDescription = null) },
                    modifier = Modifier.clickable { conflictDialogIndex = 0 },
                )
                if (current != null && conflictDialogIndex >= 0) {
                    ConflictDialog(
                        conflict = current,
                        onResolve = { conflict, choice -> controller.resolveConflict(conflict, choice) },
                        onDismiss = { conflictDialogIndex = -1 },
                    )
                }
            }
            androidx.compose.foundation.layout.Box {
                when (selectedTab) {
                    0 -> DashboardScreen(
                        missions = missions,
                        tasks = tasks,
                        conflictCount = conflictCount,
                        sync = sync,
                        error = error,
                        onViewAllMissions = { selectedTab = 1 },
                    )
                    1 -> MissionsScreen(
                        missions = missions,
                        onCreateMission = { name, description -> controller.createMission(name, description) },
                        onOpenMission = { mission -> openMission = mission },
                    )
                    2 -> TasksScreen(
                        tasks = tasks,
                        missions = missions,
                        onCreateTask = { missionId, title, description -> controller.createTask(missionId, title, description) },
                        onOpenTask = { task -> openTask = task },
                    )
                    3 -> NotificationsScreen(notifications = notifications)
                    4 -> FilesScreen(
                        onUpload = { path -> controller.uploadFile(path) },
                        onDownload = { hash, dest -> controller.downloadFile(hash, dest) },
                    )
                    5 -> {
                        val sessionPrefs = remember { SessionPreferences(context) }
                        SettingsScreen(
                            organizationId = controller.organizationId,
                            userId = controller.userId,
                            relayEndpoint = sessionPrefs.relayEndpoint,
                            missionCount = missions.size,
                            taskCount = tasks.size,
                            pendingOutboxCount = sync.pendingOutboxCount,
                            onSaveRelayEndpoint = { endpoint -> sessionPrefs.relayEndpoint = endpoint },
                            onSignOut = onSignOut,
                        )
                    }
                }
            }
        }
    }
}

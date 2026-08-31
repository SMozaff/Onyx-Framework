package com.onyx.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.TaskAlt
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import com.onyx.controller.OnyxController
import com.onyx.model.LoadedAggregate
import com.onyx.ui.screens.DashboardScreen
import com.onyx.ui.screens.MissionDetailScreen
import com.onyx.ui.screens.MissionsScreen
import com.onyx.ui.screens.NotificationsScreen
import com.onyx.ui.screens.TaskDetailScreen
import com.onyx.ui.screens.TasksScreen

/**
 * The real post-login app shell for A4, Kotlin's port of `ui/app.dart`'s
 * `_MobileShell`: a bottom navigation bar switching between the four
 * screens this task builds (Home/Missions/Tasks/Alerts, in that exact
 * order and with the same icons/labels Dart uses for these four --
 * confirmed directly against `ui/app.dart`'s real
 * `NavigationDestination` list). Approvals/Files/Settings are Dart's
 * remaining three destinations, deliberately not built here -- A5's
 * scope, not a silent gap -- so this shell only shows the first four.
 *
 * Mission/Task Detail are pushed as a full-screen overlay on top of the
 * shell (mirroring `Navigator.push`), taking a [LoadedAggregate]
 * snapshot rather than watching [controller] live -- the one real
 * exception the parity matrix documents (§12): Detail screens freeze
 * their displayed version/status as of navigation time.
 */
@Composable
fun AppShell(controller: OnyxController) {
    var selectedTab by rememberSaveable { mutableStateOf(0) }
    var openMission by remember { mutableStateOf<LoadedAggregate?>(null) }
    var openTask by remember { mutableStateOf<LoadedAggregate?>(null) }

    val missions by controller.missions.collectAsState()
    val tasks by controller.tasks.collectAsState()
    val notifications by controller.notifications.collectAsState()
    val sync by controller.sync.collectAsState()
    val conflictCount by controller.conflictCount.collectAsState()
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
        bottomBar = {
            NavigationBar {
                NavigationBarItem(selected = selectedTab == 0, onClick = { selectedTab = 0 }, icon = { Icon(Icons.Filled.Dashboard, contentDescription = null) }, label = { Text("Home") })
                NavigationBarItem(selected = selectedTab == 1, onClick = { selectedTab = 1 }, icon = { Icon(Icons.Filled.Flag, contentDescription = null) }, label = { Text("Missions") })
                NavigationBarItem(selected = selectedTab == 2, onClick = { selectedTab = 2 }, icon = { Icon(Icons.Filled.TaskAlt, contentDescription = null) }, label = { Text("Tasks") })
                NavigationBarItem(selected = selectedTab == 3, onClick = { selectedTab = 3 }, icon = { Icon(Icons.Filled.Notifications, contentDescription = null) }, label = { Text("Alerts") })
            }
        },
    ) { padding ->
        androidx.compose.foundation.layout.Box(modifier = Modifier.padding(padding)) {
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
            }
        }
    }
}

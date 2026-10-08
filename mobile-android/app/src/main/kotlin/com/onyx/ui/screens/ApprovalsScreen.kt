package com.onyx.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.TaskAlt
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.ApprovalsFilter
import com.onyx.model.LoadedAggregate
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Kotlin port of `ui/screens/approvals.dart`, re-verified fresh against
 * that file for A5.1 -- this task's own correction to A5's original
 * scoping error (A4 was incorrectly believed to have covered this).
 *
 * A queue/discovery surface only, exactly matching Dart's real,
 * current design: no Approve/Reject actions live here -- those already
 * exist on Task/Task Detail and Mission Detail (built in A4). Tapping a
 * result opens the existing detail screen for its real type; pulling to
 * refresh calls the existing [com.onyx.controller.OnyxController.refresh],
 * not a new mechanism. The filtering itself is [ApprovalsFilter], kept
 * pure and separate from this composable so the filtering logic is a
 * trivial JVM unit-test target with no Compose instrumentation needed.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ApprovalsScreen(
    tasks: List<LoadedAggregate>,
    missions: List<LoadedAggregate>,
    onRefresh: () -> Unit,
    onOpenTask: (LoadedAggregate) -> Unit,
    onOpenMission: (LoadedAggregate) -> Unit,
) {
    val pendingTasks = ApprovalsFilter.pendingTasks(tasks)
    val pendingMissions = ApprovalsFilter.pendingMissions(missions)
    val isEmpty = pendingTasks.isEmpty() && pendingMissions.isEmpty()

    var isRefreshing by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    PullToRefreshBox(
        isRefreshing = isRefreshing,
        onRefresh = {
            onRefresh()
            // OnyxController.refresh() is fire-and-forget (launches its
            // own coroutine, no completion signal this screen can await)
            // -- a short, fixed spinner window is the same real
            // limitation as any other screen driving a refresh from a
            // Compose-only signal, not a fake success state.
            isRefreshing = true
            scope.launch {
                delay(600)
                isRefreshing = false
            }
        },
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            item { Text("Approvals", style = MaterialTheme.typography.headlineSmall) }
            item {
                Text(
                    "Tasks and missions awaiting your decision.",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp, bottom = 16.dp),
                )
            }
            if (isEmpty) {
                item {
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Box(modifier = Modifier.fillMaxWidth().padding(24.dp)) {
                            Text(
                                "No tasks or missions are currently awaiting approval.",
                                modifier = Modifier.align(Alignment.Center),
                            )
                        }
                    }
                }
            } else {
                items(pendingTasks) { task ->
                    ApprovalRow(
                        icon = Icons.Filled.TaskAlt,
                        title = task.title,
                        subtitle = task.description ?: "Task ${task.id}",
                        onClick = { onOpenTask(task) },
                    )
                }
                items(pendingMissions) { mission ->
                    ApprovalRow(
                        icon = Icons.Filled.Flag,
                        title = mission.title,
                        subtitle = mission.description ?: "Mission ${mission.id}",
                        onClick = { onOpenMission(mission) },
                    )
                }
            }
        }
    }
}

@Composable
private fun ApprovalRow(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, subtitle: String, onClick: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp),
        onClick = onClick,
    ) {
        androidx.compose.foundation.layout.Row(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icon, contentDescription = null)
            androidx.compose.foundation.layout.Column(modifier = Modifier.weight(1f).padding(horizontal = 12.dp)) {
                Text(title)
                Text(subtitle, style = MaterialTheme.typography.bodySmall)
            }
            Icon(Icons.Filled.ChevronRight, contentDescription = null)
        }
    }
}

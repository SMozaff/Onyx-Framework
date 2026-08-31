package com.onyx.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.LoadedAggregate
import com.onyx.model.SyncSnapshot

/**
 * Kotlin port of `ui/screens/dashboard.dart`, re-verified fresh against
 * that file for A4 (not from memory of an earlier summary): a single
 * scrollable "command center" -- a stat row (Missions/Tasks/Conflicts/
 * Queued counts), an error card if [error] is set, a conflict-review
 * warning card if [conflictCount] is non-zero, an "Active missions"
 * section (first 3, "View all" switches the shared bottom-nav tab, not
 * a `Navigator` push -- see [onViewAllMissions]), and a "Recent
 * activity" card that is a re-slice of the same already-loaded
 * mission/task lists (first 2 of each), not a real domain event log --
 * exactly Dart's own documented behavior, reproduced here rather than
 * "improved" into a real feed this task was not asked to build.
 */
@Composable
fun DashboardScreen(
    missions: List<LoadedAggregate>,
    tasks: List<LoadedAggregate>,
    conflictCount: Int,
    sync: SyncSnapshot,
    error: String?,
    onViewAllMissions: () -> Unit,
) {
    LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
        item {
            StatRow(missions.size, tasks.size, conflictCount, sync.pendingOutboxCount)
        }
        if (error != null) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                    Text("Local core unavailable", modifier = Modifier.padding(12.dp), style = MaterialTheme.typography.titleSmall)
                    Text(error, modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp), style = MaterialTheme.typography.bodySmall)
                }
            }
        }
        if (conflictCount > 0) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                    Text(
                        "Conflict review required ($conflictCount)",
                        modifier = Modifier.padding(12.dp),
                        style = MaterialTheme.typography.titleSmall,
                    )
                }
            }
        }
        item {
            Column(modifier = Modifier.padding(vertical = 8.dp)) {
                Row2("Active missions") { TextButton(onClick = onViewAllMissions) { Text("View all") } }
            }
        }
        if (missions.isEmpty()) {
            item {
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text("No missions yet")
                        Text("Create the first mission from the Missions tab.", style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        } else {
            items(missions.take(3)) { mission ->
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Text(mission.title, modifier = Modifier.padding(12.dp))
                }
            }
        }
        item {
            Card(modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text("Recent activity", style = MaterialTheme.typography.titleSmall)
                    if (missions.isEmpty() && tasks.isEmpty()) {
                        Text("Domain events will appear here...", style = MaterialTheme.typography.bodySmall)
                    } else {
                        missions.take(2).forEach { m -> Text("Mission ${m.status} · version ${m.version}") }
                        tasks.take(2).forEach { t -> Text("Task ${t.status} · version ${t.version}") }
                    }
                }
            }
        }
    }
}

@Composable
private fun StatRow(missionCount: Int, taskCount: Int, conflictCount: Int, queuedCount: Int) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row2("Overview") {}
        androidx.compose.foundation.layout.Row(horizontalArrangement = Arrangement.SpaceBetween, modifier = Modifier.fillMaxWidth()) {
            StatTile("Missions", missionCount)
            StatTile("Tasks", taskCount)
            StatTile("Conflicts", conflictCount)
            StatTile("Queued", queuedCount)
        }
    }
}

@Composable
private fun StatTile(label: String, count: Int) {
    Column {
        Text(count.toString(), style = MaterialTheme.typography.headlineSmall)
        Text(label, style = MaterialTheme.typography.bodySmall)
    }
}

@Composable
private fun Row2(title: String, trailing: @Composable () -> Unit) {
    androidx.compose.foundation.layout.Row(
        horizontalArrangement = Arrangement.SpaceBetween,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        trailing()
    }
}

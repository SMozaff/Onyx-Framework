package com.onyx.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.LoadedAggregate
import kotlinx.coroutines.launch

/**
 * Kotlin port of `ui/screens/tasks.dart`, re-verified fresh for A4:
 * full, unfiltered `tasks` list; a create-task FAB. Critically, if
 * [missions] is empty when the FAB is tapped, the create dialog is
 * never shown -- a `SnackBar` ("Create a mission before adding tasks.")
 * is shown instead, exactly matching Dart's real, client-side-only
 * enforcement (the FAB itself is never disabled).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TasksScreen(
    tasks: List<LoadedAggregate>,
    missions: List<LoadedAggregate>,
    onCreateTask: (missionId: String, title: String, description: String?) -> Unit,
    onOpenTask: (LoadedAggregate) -> Unit,
) {
    var showCreateDialog by remember { mutableStateOf(false) }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                text = { Text("Task") },
                icon = {},
                onClick = {
                    if (missions.isEmpty()) {
                        scope.launch { snackbarHostState.showSnackbar("Create a mission before adding tasks.") }
                    } else {
                        showCreateDialog = true
                    }
                },
            )
        },
    ) { padding ->
        Box(modifier = Modifier.fillMaxSize().padding(padding)) {
            if (tasks.isEmpty()) {
                Text("No tasks are stored on this device.", modifier = Modifier.align(Alignment.Center).padding(24.dp))
            } else {
                LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                    items(tasks) { task ->
                        Card(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                            onClick = { onOpenTask(task) },
                        ) {
                            Text(task.title, modifier = Modifier.padding(12.dp))
                            Text("Status: ${task.status}", modifier = Modifier.padding(horizontal = 12.dp), style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
        }
    }

    if (showCreateDialog && missions.isNotEmpty()) {
        var selectedMission by remember { mutableStateOf(missions.first()) }
        var expanded by remember { mutableStateOf(false) }
        var title by remember { mutableStateOf("") }
        var description by remember { mutableStateOf("") }

        AlertDialog(
            onDismissRequest = { showCreateDialog = false },
            title = { Text("New Task") },
            text = {
                androidx.compose.foundation.layout.Column {
                    Text("Mission", style = MaterialTheme.typography.labelSmall)
                    androidx.compose.foundation.layout.Box {
                        OutlinedButton(onClick = { expanded = true }) { Text(selectedMission.title) }
                        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                            missions.forEach { mission ->
                                DropdownMenuItem(text = { Text(mission.title) }, onClick = { selectedMission = mission; expanded = false })
                            }
                        }
                    }
                    OutlinedTextField(value = title, onValueChange = { title = it }, label = { Text("Title") }, singleLine = true)
                    OutlinedTextField(value = description, onValueChange = { description = it }, label = { Text("Description") }, singleLine = true)
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    showCreateDialog = false
                    if (title.trim().isNotEmpty()) {
                        onCreateTask(selectedMission.id, title.trim(), description.trim().ifEmpty { null })
                    }
                }) { Text("Create") }
            },
            dismissButton = { TextButton(onClick = { showCreateDialog = false }) { Text("Cancel") } },
        )
    }
}

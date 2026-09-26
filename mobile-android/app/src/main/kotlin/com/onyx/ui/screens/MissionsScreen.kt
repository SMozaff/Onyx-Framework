package com.onyx.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.LoadedAggregate

/**
 * Kotlin port of `ui/screens/missions.dart`, re-verified fresh for A4:
 * full, unfiltered `missions` list (no pagination), pull-to-refresh
 * (here: pull-to-refresh is a later polish item -- the FAB and list are
 * the real parity surface this task targets), and a create-mission FAB
 * opening a dialog with Name (required) and Description (optional).
 * Submitting with a blank name silently does nothing, matching Dart's
 * own "dialog already closed, no error surfaced" behavior exactly.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MissionsScreen(
    missions: List<LoadedAggregate>,
    onCreateMission: (name: String, description: String?) -> Unit,
    onOpenMission: (LoadedAggregate) -> Unit,
) {
    var showCreateDialog by remember { mutableStateOf(false) }

    Scaffold(
        floatingActionButton = {
            ExtendedFloatingActionButton(onClick = { showCreateDialog = true }, text = { Text("Mission") }, icon = {})
        },
    ) { padding ->
        Box(modifier = Modifier.fillMaxSize().padding(padding)) {
            if (missions.isEmpty()) {
                Text(
                    "No missions are stored on this device.",
                    modifier = Modifier.align(Alignment.Center).padding(24.dp),
                )
            } else {
                LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                    items(missions) { mission ->
                        Card(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                            onClick = { onOpenMission(mission) },
                        ) {
                            Text(mission.title, modifier = Modifier.padding(12.dp))
                            mission.description?.let { Text(it, modifier = Modifier.padding(horizontal = 12.dp, vertical = 0.dp), style = MaterialTheme.typography.bodySmall) }
                        }
                    }
                }
            }
        }
    }

    if (showCreateDialog) {
        var name by remember { mutableStateOf("") }
        var description by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { showCreateDialog = false },
            title = { Text("New Mission") },
            text = {
                androidx.compose.foundation.layout.Column {
                    OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Name") }, singleLine = true)
                    OutlinedTextField(value = description, onValueChange = { description = it }, label = { Text("Description") }, singleLine = true)
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    showCreateDialog = false
                    if (name.trim().isNotEmpty()) {
                        onCreateMission(name.trim(), description.trim().ifEmpty { null })
                    }
                }) { Text("Create") }
            },
            dismissButton = { TextButton(onClick = { showCreateDialog = false }) { Text("Cancel") } },
        )
    }
}

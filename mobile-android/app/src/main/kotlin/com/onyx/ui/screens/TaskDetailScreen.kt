package com.onyx.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.controller.OnyxController
import com.onyx.model.LoadedAggregate
import kotlinx.coroutines.launch

/**
 * Kotlin port of `ui/screens/task_detail.dart`, re-verified fresh for
 * A4: structurally identical to Mission Detail, but `canDecide` gates
 * on task status `"Submitted"` specifically (not `"AwaitingApproval"`
 * -- the two aggregates use different status vocabularies for
 * "awaiting a decision"), and the real commands are `RejectTask`/
 * `ApproveTask` -- a genuinely different pair of names from Mission's
 * `RejectApproval`/`ActivateMission`, confirmed directly in
 * `task_detail.dart`, not assumed symmetric.
 *
 * Reject requires a non-empty reason; Approve does not.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaskDetailScreen(task: LoadedAggregate, controller: OnyxController, onBack: () -> Unit) {
    var reason by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val canDecide = task.status == "Submitted"

    Scaffold(topBar = { TopAppBar(title = { Text(task.title) }) }) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp)) {
            task.description?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
            Text("Status: ${task.status}", style = MaterialTheme.typography.bodySmall)

            Card(modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp)) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text("Execution state", style = MaterialTheme.typography.titleSmall)
                    Text("Version: ${task.version}")
                    Text("Lifecycle epoch: ${task.lifecycleEpoch}")
                    Text("Authority epoch: ${task.authorityEpoch}")
                    Text("ID: ${task.id}")
                }
            }

            if (canDecide) {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text("Review submission", style = MaterialTheme.typography.titleSmall)
                        OutlinedTextField(
                            value = reason,
                            onValueChange = { reason = it },
                            label = { Text("Reason") },
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Row(modifier = Modifier.padding(top = 8.dp)) {
                            OutlinedButton(
                                enabled = !busy && reason.trim().isNotEmpty(),
                                onClick = {
                                    busy = true
                                    scope.launch {
                                        try {
                                            controller.decide(task, "task", "RejectTask", reason.trim())
                                            onBack()
                                        } catch (e: Exception) {
                                            error = e.message ?: e.toString()
                                        } finally {
                                            busy = false
                                        }
                                    }
                                },
                            ) { Text("Reject") }
                            Button(
                                enabled = !busy,
                                onClick = {
                                    busy = true
                                    scope.launch {
                                        try {
                                            controller.decide(task, "task", "ApproveTask", reason.trim())
                                            onBack()
                                        } catch (e: Exception) {
                                            error = e.message ?: e.toString()
                                        } finally {
                                            busy = false
                                        }
                                    }
                                },
                                modifier = Modifier.padding(start = 8.dp),
                            ) { Text("Approve") }
                        }
                        error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                    }
                }
            }
        }
    }
}

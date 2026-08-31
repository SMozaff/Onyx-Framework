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
 * Kotlin port of `ui/screens/mission_detail.dart`, re-verified fresh
 * for A4: takes a [mission] snapshot (as of navigation time, not live --
 * matching Dart's `LoadedAggregate` navigation-argument pattern exactly,
 * per the parity matrix's own note that Detail screens are the one
 * partial exception to "every screen watches the shared controller
 * live"). Shows an "Authority state" card (version/lifecycle epoch/
 * authority epoch/id) and, only when `status == "AwaitingApproval"`
 * (`canDecide`), a decision card with Reject/Activate -- Mission's real
 * command pair is `RejectApproval`/`ActivateMission`, NOT a direct
 * `ApproveMission` mirror of Task's shape (confirmed directly in
 * `mission_detail.dart`, not assumed symmetric with Task).
 *
 * Reject requires a non-empty reason; Activate does not.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MissionDetailScreen(mission: LoadedAggregate, controller: OnyxController, onBack: () -> Unit) {
    var reason by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val canDecide = mission.status == "AwaitingApproval"

    Scaffold(topBar = { TopAppBar(title = { Text(mission.title) }) }) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp)) {
            mission.description?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
            Text("Status: ${mission.status}", style = MaterialTheme.typography.bodySmall)

            Card(modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp)) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text("Authority state", style = MaterialTheme.typography.titleSmall)
                    Text("Version: ${mission.version}")
                    Text("Lifecycle epoch: ${mission.lifecycleEpoch}")
                    Text("Authority epoch: ${mission.authorityEpoch}")
                    Text("ID: ${mission.id}")
                }
            }

            if (canDecide) {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text("Review approval request", style = MaterialTheme.typography.titleSmall)
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
                                            controller.decide(mission, "mission", "RejectApproval", reason.trim())
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
                                            controller.decide(mission, "mission", "ActivateMission", reason.trim())
                                            onBack()
                                        } catch (e: Exception) {
                                            error = e.message ?: e.toString()
                                        } finally {
                                            busy = false
                                        }
                                    }
                                },
                                modifier = Modifier.padding(start = 8.dp),
                            ) { Text("Activate") }
                        }
                        error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                    }
                }
            }
        }
    }
}

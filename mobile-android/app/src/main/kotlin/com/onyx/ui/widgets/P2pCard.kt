package com.onyx.ui.widgets

import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.onyx.p2p.P2pStatus
import com.onyx.p2p.P2pTransport
import com.onyx.p2p.P2pViewModel

/**
 * The Settings-surface P2P session card (MIGRATION_PLAN Phase 4.1 / the
 * contract's P2P wiring row): transport toggle, the two-role controls
 * the controller exposes (BLE server vs. client scan + connect / Wi-Fi
 * Direct discover + connect), a probe-message composer fed through the
 * Rust codec channel, and a live transcript of received/sent messages.
 *
 * Note on the editable field: this card deliberately uses `TextField`,
 * not `OutlinedTextField`, because `SettingsScreenSourceTest` pins the
 * whole Settings screen to exactly one `OutlinedTextField` (the relay
 * endpoint) — a hard, enforced constraint this card must not disturb.
 * Runtime permissions are requested on first action via the Activity
 * result contract, gated on the transport union the controller
 * reports; the controls themselves stay honest about un-granted
 * permissions through [P2pStatus.Error].
 */
@Composable
fun P2pCard(viewModel: P2pViewModel) {
    val context = LocalContext.current
    val status by viewModel.status.collectAsState()
    val peers by viewModel.peers.collectAsState()
    val messages by viewModel.messages.collectAsState()

    var transport by remember { mutableStateOf(P2pTransport.BLE) }
    var probe by remember { mutableStateOf("") }
    var pendingAction by remember { mutableStateOf<(() -> Unit)?>(null) }
    val activity = context as? ComponentActivity

    val launcher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { granted ->
        val action = pendingAction
        pendingAction = null
        if (granted.values.all { it }) action?.invoke() else viewModel.stop()
    }

    fun requestPermissionsAndRun(required: List<String>, action: () -> Unit) {
        if (activity == null) return
        val missing = required.filterNot {
            ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
        }
        if (missing.isEmpty()) {
            action()
        } else {
            pendingAction = action
            launcher.launch(missing.toTypedArray())
        }
    }

    Card(modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp)) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Peer-to-peer sync", style = MaterialTheme.typography.titleSmall)
            Text(
                "A direct device-to-device link through the Rust transport codec (Phase 4.1). " +
                    "Needs two devices and the runtime radio permission; message framing and encryption " +
                    "are the native codec's, not this screen's.",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp),
            )

            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            ) {
                OutlinedButton(
                    onClick = { transport = P2pTransport.BLE },
                ) { Text("BLE") }
                OutlinedButton(
                    onClick = { transport = P2pTransport.WIFI_DIRECT },
                ) { Text("Wi-Fi Direct") }
            }

            when (val current = status) {
                is P2pStatus.Connected ->
                    Text("Connected to ${current.peer}", style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 8.dp))
                is P2pStatus.Error ->
                    Text(current.message, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 8.dp))
                else ->
                    Text(transport.label, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 8.dp))
            }

            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            ) {
                Button(onClick = {
                    requestPermissionsAndRun(viewModel.permissionsFor(transport)) { viewModel.startServer() }
                }) { Text("Start server (accept)") }
                if (transport == P2pTransport.BLE) {
                    Button(onClick = {
                        requestPermissionsAndRun(viewModel.permissionsFor(transport)) { viewModel.connectBle() }
                    }) { Text("Scan & connect") }
                } else {
                    Button(onClick = {
                        requestPermissionsAndRun(viewModel.permissionsFor(transport)) { viewModel.discoverWifiPeers() }
                    }) { Text("Discover") }
                }
                OutlinedButton(onClick = { viewModel.stop() }) { Text("Stop") }
            }

            if (transport == P2pTransport.WIFI_DIRECT && peers.isNotEmpty()) {
                LazyColumn(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    items(peers) { peer ->
                        Row(
                            horizontalArrangement = Arrangement.SpaceBetween,
                            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                        ) {
                            Text(peer.name, style = MaterialTheme.typography.bodyMedium)
                            OutlinedButton(onClick = {
                                requestPermissionsAndRun(viewModel.permissionsFor(transport)) { viewModel.connectWifi(peer) }
                            }) { Text("Connect") }
                        }
                    }
                }
            }

            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            ) {
                TextField(
                    value = probe,
                    onValueChange = { probe = it },
                    label = { Text("Probe message") },
                    modifier = Modifier.weight(1f),
                )
                Button(
                    enabled = status is P2pStatus.Connected,
                    onClick = {
                        if (probe.isNotBlank() && viewModel.send(probe)) probe = ""
                    },
                ) { Text("Send") }
            }

            if (messages.isNotEmpty()) {
                Column(modifier = Modifier.padding(top = 12.dp)) {
                    messages.takeLast(6).forEach { m ->
                        Text(
                            (if (m.incoming) "← " else "→ ") + m.text,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }
    }
}
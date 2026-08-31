package com.onyx.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Card
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Kotlin port of `ui/screens/settings.dart`, re-verified fresh against
 * that file for A5. This app has no HTTP-transport mode to mirror (A1
 * never built one -- FFI/local-first is the only mode), so Dart's
 * "Connection mode" (LAN vs. local-first) card has no Kotlin equivalent
 * here; everything that *does* apply is preserved exactly.
 *
 * # The one real security property this screen must never regress
 * `organization_id`/`user_id` are shown strictly **read-only** --
 * `Text`, never `OutlinedTextField`/any other editable control -- and
 * [relayEndpoint] is the *only* field a person can edit here. This is
 * not a style choice: Dart's own doc comment records a real, already-
 * fixed security hole (`settings.dart`, and independently again in
 * `startup_error_screen.dart`) -- these fields used to be free-text,
 * letting anyone type an arbitrary organization/user UUID and have
 * `mobile-core` act as it, with zero connection to a real login.
 * Identity now only ever changes via a real login or [onSignOut],
 * never via text entry on this screen. `SettingsScreenSourceTest`
 * (`src/test/kotlin/com/onyx/ui/screens/SettingsScreenSourceTest.kt`)
 * is a real, direct, automated proof of this property -- not just this
 * doc comment's claim -- reading this file's own source at test time.
 */
@Composable
fun SettingsScreen(
    organizationId: String,
    userId: String,
    relayEndpoint: String,
    missionCount: Int,
    taskCount: Int,
    pendingOutboxCount: Int,
    onSaveRelayEndpoint: (String) -> Unit,
    onSignOut: () -> Unit,
) {
    var relay by remember(relayEndpoint) { mutableStateOf(relayEndpoint) }
    var saveMessage by remember { mutableStateOf<String?>(null) }

    LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
        item { Text("Settings", style = MaterialTheme.typography.headlineSmall) }

        item {
            Card(modifier = Modifier.fillMaxWidth().padding(vertical = 16.dp)) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Signed in", style = MaterialTheme.typography.titleSmall)
                    Text(
                        "Organization: $organizationId\nUser: $userId",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    Text(
                        "To act as a different account, use \"Sign out\" below and sign in again " +
                            "— this can no longer be changed by editing it directly.",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    OutlinedTextField(
                        value = relay,
                        onValueChange = { relay = it },
                        label = { Text("Cloud relay endpoint") },
                        modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                    )
                    androidx.compose.foundation.layout.Row(
                        horizontalArrangement = Arrangement.End,
                        modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                    ) {
                        Button(onClick = {
                            onSaveRelayEndpoint(relay.trim())
                            saveMessage = "Cloud relay endpoint saved. Restart the app to apply it."
                        }) { Text("Save") }
                    }
                    if (saveMessage != null) {
                        Text(saveMessage!!, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 8.dp))
                    }
                }
            }
        }

        item {
            Card(modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp)) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Local-first database", style = MaterialTheme.typography.titleSmall)
                    Text(
                        "$missionCount missions · $taskCount tasks · $pendingOutboxCount queued events",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }

        item {
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Account", style = MaterialTheme.typography.titleSmall)
                    Text(
                        "Signing out clears your saved login on this device — local data and sync " +
                            "history stay put, but you'll need to sign in again to resume Task/Mission " +
                            "approvals and syncing under your account.",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    androidx.compose.foundation.layout.Row(
                        horizontalArrangement = Arrangement.End,
                        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    ) {
                        OutlinedButton(onClick = onSignOut) { Text("Sign out") }
                    }
                }
            }
        }
    }
}

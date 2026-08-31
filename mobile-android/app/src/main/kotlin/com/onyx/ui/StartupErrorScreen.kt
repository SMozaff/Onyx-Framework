package com.onyx.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Startup-failure recovery screen for A3, Kotlin's equivalent of Dart's
 * `startup_error_screen.dart`. Carries forward, deliberately (not
 * independently rediscovered — see `OnyxSessionViewModel`'s own
 * top-level doc comment), the one real security property that file's
 * own history establishes: **no editable organization/user-id field,
 * ever.** The only two actions here are "Retry" (try the exact same
 * startup sequence again -- covers a transient failure) and "Sign out
 * and retry" (clear the saved identity/session and fall back to a real
 * login -- covers a corrupted/stale saved identity). Neither can ever
 * let someone act as an organization/user they have not actually
 * authenticated as.
 */
@Composable
fun StartupErrorScreen(
    message: String,
    technicalDetail: String,
    onRetry: () -> Unit,
    onSignOutAndRetry: () -> Unit,
) {
    var showDetails by remember { mutableStateOf(false) }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
            Text("ONYX couldn't start", style = MaterialTheme.typography.headlineSmall)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(8.dp))
            Text(message, style = MaterialTheme.typography.bodyMedium)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(8.dp))

            Button(onClick = { showDetails = !showDetails }) {
                Text(if (showDetails) "Hide technical details" else "Show technical details")
            }
            if (showDetails) {
                androidx.compose.foundation.layout.Spacer(Modifier.padding(8.dp))
                Text(technicalDetail, style = MaterialTheme.typography.bodySmall)
            }

            androidx.compose.foundation.layout.Spacer(Modifier.padding(16.dp))
            Button(onClick = onRetry, modifier = Modifier.fillMaxWidth()) {
                Text("Retry")
            }
            androidx.compose.foundation.layout.Spacer(Modifier.padding(4.dp))
            OutlinedButton(onClick = onSignOutAndRetry, modifier = Modifier.fillMaxWidth()) {
                Text("Sign out and retry")
            }
        }
    }
}

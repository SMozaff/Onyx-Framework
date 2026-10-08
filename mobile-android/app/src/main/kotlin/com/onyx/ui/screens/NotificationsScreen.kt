package com.onyx.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.LoadedAggregate

/**
 * Kotlin port of `ui/screens/notifications.dart`, re-verified fresh for
 * A4: a direct, minimal read-only list -- no tap action, no create/
 * decide actions, matching Dart's own intentionally minimal 40-line
 * reference exactly (no scope added beyond what that file has). Empty
 * state text is copied verbatim, including its own disclosed caveat
 * that no local flow currently populates this aggregate type on-device.
 */
@Composable
fun NotificationsScreen(notifications: List<LoadedAggregate>) {
    Box(modifier = Modifier.fillMaxSize()) {
        if (notifications.isEmpty()) {
            Text(
                "No local Notification aggregate is available yet. Remote notification delivery " +
                    "remains available through the web client.",
                modifier = Modifier.align(Alignment.Center).padding(24.dp),
            )
        } else {
            LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                items(notifications) { notification ->
                    Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                        Text(notification.title, modifier = Modifier.padding(12.dp))
                        Text(
                            notification.description ?: notification.id,
                            modifier = Modifier.padding(horizontal = 12.dp),
                            style = MaterialTheme.typography.bodySmall,
                        )
                        Text(
                            "Status: ${notification.status}",
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                            style = MaterialTheme.typography.labelSmall,
                        )
                    }
                }
            }
        }
    }
}

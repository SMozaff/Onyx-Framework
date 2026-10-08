package com.onyx.ui.widgets

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CloudDone
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.Devices
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material.icons.filled.WarningAmber
import androidx.compose.material3.AssistChip
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.onyx.model.SyncSnapshot

/**
 * Kotlin port of `ui/widgets/sync_status.dart`, re-verified fresh
 * against that file for A5. Reads from the shared controller state
 * ([snapshot]/[syncing]/[hasNetwork] passed in from
 * [com.onyx.controller.OnyxController]'s own single-refresh cycle) --
 * this widget never independently queries `mobile-core`, matching A4's
 * established architectural pattern.
 *
 * Same real label/icon/color precedence as Dart's own `switch`
 * expression, checked field-for-field: Syncing > Conflict > Queued N >
 * Online > Local (has device network, but `mobile-core` reports
 * offline) > Offline.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncStatusIndicator(
    snapshot: SyncSnapshot,
    syncing: Boolean,
    hasNetwork: Boolean,
    onClick: () -> Unit,
) {
    val (label, icon, color) = when {
        syncing -> Triple("Syncing", Icons.Filled.Sync, Color(0xFF1A8CBF))
        snapshot.openConflictCount > 0 -> Triple("Conflict", Icons.Filled.WarningAmber, Color(0xFFC52A2A))
        snapshot.pendingOutboxCount > 0 -> Triple("Queued ${snapshot.pendingOutboxCount}", Icons.Filled.Schedule, Color(0xFFD4A020))
        snapshot.online -> Triple("Online", Icons.Filled.CloudDone, Color(0xFF1A8C38))
        hasNetwork -> Triple("Local", Icons.Filled.Devices, Color(0xFF1A8CBF))
        else -> Triple("Offline", Icons.Filled.CloudOff, Color(0xFFC52A2A))
    }
    AssistChip(
        onClick = onClick,
        enabled = !syncing,
        label = { Text(label) },
        leadingIcon = {
            if (syncing) {
                CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
            } else {
                Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(17.dp))
            }
        },
        modifier = Modifier.semantics { contentDescription = "Synchronization status: $label. Tap to synchronize now." },
    )
}

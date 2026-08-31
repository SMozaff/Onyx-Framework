package com.onyx.ui.widgets

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.model.ConflictChoice
import com.onyx.model.SyncConflict
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * Kotlin port of `ui/widgets/conflict_dialog.dart`, re-verified fresh
 * against that file for A5. All three of Dart's real resolution choices
 * -- accept local, accept remote, escalate -- go through
 * [onResolve] (Kotlin's equivalent of
 * `context.read<OnyxController>().resolveConflict`), not a subset.
 */
@Composable
fun ConflictDialog(
    conflict: SyncConflict,
    onResolve: suspend (SyncConflict, ConflictChoice) -> Unit,
    onDismiss: () -> Unit,
) {
    var resolving by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    fun resolve(choice: ConflictChoice) {
        resolving = true
        scope.launch {
            try {
                onResolve(conflict, choice)
                onDismiss()
            } finally {
                resolving = false
            }
        }
    }

    AlertDialog(
        onDismissRequest = { if (!resolving) onDismiss() },
        title = { Text("Conflict detected") },
        text = {
            Column(modifier = Modifier.width(320.dp).verticalScroll(rememberScrollState())) {
                Text("Field: ${conflict.fieldPath}", style = MaterialTheme.typography.titleSmall)
                ComparisonPanel(label = "Local", value = conflict.localValue)
                ComparisonPanel(label = "Remote", value = conflict.remoteValue)
            }
        },
        confirmButton = {
            androidx.compose.material3.Button(enabled = !resolving, onClick = { resolve(ConflictChoice.LOCAL) }) {
                Text("Accept local")
            }
        },
        dismissButton = {
            Column {
                OutlinedButton(enabled = !resolving, onClick = { resolve(ConflictChoice.REMOTE) }) { Text("Accept remote") }
                TextButton(enabled = !resolving, onClick = { resolve(ConflictChoice.ESCALATE) }) { Text("Escalate") }
            }
        },
    )
}

@Composable
private fun ComparisonPanel(label: String, value: Any?) {
    Column(
        modifier = Modifier
            .padding(vertical = 6.dp)
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(12.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge)
        SelectionContainer {
            Text(if (value is String) value else valueToDisplayString(value), modifier = Modifier.padding(top = 6.dp))
        }
    }
}

private fun valueToDisplayString(value: Any?): String = when (value) {
    null -> "null"
    is JSONObject -> value.toString(2)
    else -> value.toString()
}

package com.onyx.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * Kotlin port of `ui/screens/files.dart`, re-verified fresh against that
 * file for A5: a real upload/download UI over
 * [com.onyx.controller.OnyxController.uploadFile]/[com.onyx.controller.OnyxController.downloadFile]
 * (`mobile_core_upload_file`/`_download_file` on the A1 JNI adapter).
 *
 * Takes a filesystem path rather than a file-picker, matching Dart's own
 * documented reason exactly: no file-picker dependency exists in this
 * app either, and a real "pick a file" UI is Dart's own flagged
 * follow-up, not something to build differently here.
 *
 * On failure, both actions surface the same real, current *generic*
 * error [com.onyx.controller.OnyxController.uploadFile]/[downloadFile]
 * throw -- `mobile_core_upload_file`/`_download_file` collapse every
 * failure mode (an unreadable file, a file over the 100 MiB
 * `MAX_FILE_SIZE_BYTES` limit, no stored content for a hash, a write
 * error) into a null/`-1` return with no further detail, exactly like
 * Dart's own `catch (error) { 'Upload failed: $error' }` gets nothing
 * more specific than a generic `StateError`. Matching that honestly is
 * real parity with the reference's actual behavior, not a regression.
 */
@Composable
fun FilesScreen(
    onUpload: suspend (path: String) -> JSONObject,
    onDownload: suspend (contentHash: String, destinationPath: String) -> Long,
) {
    var uploadPath by remember { mutableStateOf("") }
    var downloadHash by remember { mutableStateOf("") }
    var downloadDest by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    LazyColumn(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
        item { Text("Files", style = MaterialTheme.typography.headlineSmall) }
        item {
            Card(modifier = Modifier.fillMaxWidth().padding(vertical = 16.dp)) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Upload a file", style = MaterialTheme.typography.titleSmall)
                    OutlinedTextField(
                        value = uploadPath,
                        onValueChange = { uploadPath = it },
                        label = { Text("File path on this device") },
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    Button(
                        enabled = !busy,
                        onClick = {
                            val path = uploadPath.trim()
                            if (path.isEmpty()) return@Button
                            busy = true
                            status = null
                            scope.launch {
                                try {
                                    val outcome = onUpload(path)
                                    status = "Uploaded ${outcome.optLong("size_bytes")} bytes. Content hash: ${outcome.optString("content_hash")}"
                                } catch (error: Exception) {
                                    status = "Upload failed: ${error.message ?: error}"
                                } finally {
                                    busy = false
                                }
                            }
                        },
                        modifier = Modifier.padding(top = 8.dp),
                    ) { Text("Upload") }
                }
            }
        }
        item {
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Download a file", style = MaterialTheme.typography.titleSmall)
                    OutlinedTextField(
                        value = downloadHash,
                        onValueChange = { downloadHash = it },
                        label = { Text("Content hash") },
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    OutlinedTextField(
                        value = downloadDest,
                        onValueChange = { downloadDest = it },
                        label = { Text("Save to path on this device") },
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    Button(
                        enabled = !busy,
                        onClick = {
                            val hash = downloadHash.trim()
                            val dest = downloadDest.trim()
                            if (hash.isEmpty() || dest.isEmpty()) return@Button
                            busy = true
                            status = null
                            scope.launch {
                                try {
                                    val bytesWritten = onDownload(hash, dest)
                                    status = "Downloaded $bytesWritten bytes to $dest"
                                } catch (error: Exception) {
                                    status = "Download failed: ${error.message ?: error}"
                                } finally {
                                    busy = false
                                }
                            }
                        },
                        modifier = Modifier.padding(top = 8.dp),
                    ) { Text("Download") }
                }
            }
        }
        if (status != null) {
            item { Text(status!!, modifier = Modifier.padding(top = 16.dp)) }
        }
    }
}

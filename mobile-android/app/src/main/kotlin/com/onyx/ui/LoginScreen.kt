package com.onyx.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.onyx.session.SessionPreferences

private const val CLOUD_DEFAULT_SERVER_ADDRESS = "https://onyx-framework.soheil-mozaffari.workers.dev"
private const val LOCAL_DEFAULT_SERVER_ADDRESS = "http://10.0.2.2:3000"

/**
 * Returns the persisted server address when it is a real custom endpoint,
 * while upgrading the old/blank default to the live Cloudflare Worker.
 */
fun defaultServerAddressFor(preferences: SessionPreferences): String {
    val configured = preferences.serverAddress.trim()
    return when {
        configured.isBlank() -> CLOUD_DEFAULT_SERVER_ADDRESS
        configured == LOCAL_DEFAULT_SERVER_ADDRESS -> LOCAL_DEFAULT_SERVER_ADDRESS
        configured == "http://localhost:3000" -> LOCAL_DEFAULT_SERVER_ADDRESS
        configured == "http://127.0.0.1:3000" -> LOCAL_DEFAULT_SERVER_ADDRESS
        configured.contains("onyx-api-docker.onrender.com", ignoreCase = true) -> CLOUD_DEFAULT_SERVER_ADDRESS
        else -> configured
    }
}

/** Real login screen using the Cloudflare-hosted ONYX API by default. */
@Composable
fun LoginScreen(
    defaultServerAddress: String,
    isLoggingIn: Boolean,
    errorMessage: String?,
    onLogin: (serverAddress: String, username: String, password: String) -> Unit,
) {
    var environment by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) "cloud" else "local") }
    var localAddress by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) LOCAL_DEFAULT_SERVER_ADDRESS else defaultServerAddress) }
    var cloudAddress by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) defaultServerAddress.ifBlank { CLOUD_DEFAULT_SERVER_ADDRESS } else CLOUD_DEFAULT_SERVER_ADDRESS) }
    var serverAddress by remember { mutableStateOf(defaultServerAddress.ifBlank { CLOUD_DEFAULT_SERVER_ADDRESS }) }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().padding(24.dp).widthIn(max = 420.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("ONYX — Sign in", style = MaterialTheme.typography.headlineSmall)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(4.dp))
            Text("Sign in with your real ONYX credentials.", style = MaterialTheme.typography.bodySmall)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(12.dp))
            errorMessage?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
            androidx.compose.foundation.layout.Spacer(Modifier.padding(8.dp))
            androidx.compose.foundation.layout.Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                OutlinedButton(onClick = { environment = "local"; serverAddress = localAddress }, modifier = Modifier.weight(1f)) { Text(if (environment == "local") "✓ Local Backend" else "Local Backend") }
                OutlinedButton(onClick = { environment = "cloud"; serverAddress = cloudAddress }, modifier = Modifier.weight(1f)) { Text(if (environment == "cloud") "✓ Cloud Backend" else "Cloud Backend") }
            }
            androidx.compose.foundation.layout.Spacer(Modifier.padding(4.dp))
            OutlinedTextField(value = serverAddress, onValueChange = { serverAddress = it; if (environment == "local") localAddress = it else cloudAddress = it }, label = { Text(if (environment == "cloud") "Cloud API URL" else "Local API URL (emulator uses 10.0.2.2)") }, modifier = Modifier.fillMaxWidth(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri), singleLine = true)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(6.dp))
            OutlinedTextField(value = username, onValueChange = { username = it }, label = { Text("Username or email") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(6.dp))
            OutlinedTextField(value = password, onValueChange = { password = it }, label = { Text("Password") }, modifier = Modifier.fillMaxWidth(), singleLine = true, visualTransformation = PasswordVisualTransformation())
            androidx.compose.foundation.layout.Spacer(Modifier.padding(12.dp))
            Button(onClick = { onLogin(serverAddress.trim(), username.trim(), password) }, enabled = !isLoggingIn && username.isNotBlank() && password.isNotEmpty(), modifier = Modifier.fillMaxWidth()) {
                if (isLoggingIn) CircularProgressIndicator(modifier = Modifier.padding(2.dp)) else Text("Sign in")
            }
        }
    }
}

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

/** Real login screen using the Cloudflare-hosted ONYX API by default. */
@Composable
fun LoginScreen(
    defaultServerAddress: String,
    isLoggingIn: Boolean,
    errorMessage: String?,
    onLogin: (serverAddress: String, username: String, password: String) -> Unit,
) {
    val cloudDefault = "https://onyx-framework.soheil-mozaffari.workers.dev"
    val localDefault = "http://10.0.2.2:3000"
    var environment by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) "cloud" else "local") }
    var localAddress by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) localDefault else defaultServerAddress) }
    var cloudAddress by remember { mutableStateOf(if (defaultServerAddress.startsWith("https://") || defaultServerAddress.isBlank()) defaultServerAddress.ifBlank { cloudDefault } else cloudDefault) }
    var serverAddress by remember { mutableStateOf(defaultServerAddress.ifBlank { cloudDefault }) }
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
            OutlinedTextField(value = username, onValueChange = { username = it }, label = { Text("Username") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(6.dp))
            OutlinedTextField(value = password, onValueChange = { password = it }, label = { Text("Password") }, modifier = Modifier.fillMaxWidth(), singleLine = true, visualTransformation = PasswordVisualTransformation())
            androidx.compose.foundation.layout.Spacer(Modifier.padding(12.dp))
            Button(onClick = { onLogin(serverAddress.trim(), username.trim(), password) }, enabled = !isLoggingIn && username.isNotBlank() && password.isNotEmpty(), modifier = Modifier.fillMaxWidth()) {
                if (isLoggingIn) CircularProgressIndicator(modifier = Modifier.padding(2.dp)) else Text("Sign in")
            }
        }
    }
}

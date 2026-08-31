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

/**
 * Real login screen for A3, Kotlin's equivalent of Dart's
 * `ffi_login_screen.dart`: a real `POST /api/auth/login` against
 * `api-server`, not a locally-invented identity. Field layout
 * (server address / username / password) and the specific
 * `MOBILE_ACCESS_RESTRICTED` vs. generic-failure error distinction
 * mirror that screen precisely -- see `OnyxSessionViewModel.login`'s
 * own doc comment for the exact persistence sequence this triggers on
 * success.
 */
@Composable
fun LoginScreen(
    defaultServerAddress: String,
    isLoggingIn: Boolean,
    errorMessage: String?,
    onLogin: (serverAddress: String, username: String, password: String) -> Unit,
) {
    var serverAddress by remember { mutableStateOf(defaultServerAddress) }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp)
                .widthIn(max = 420.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("ONYX — Sign in", style = MaterialTheme.typography.headlineSmall)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(4.dp))
            Text(
                "Sign in with your real ONYX credentials to confirm your identity and organization.",
                style = MaterialTheme.typography.bodySmall,
            )
            androidx.compose.foundation.layout.Spacer(Modifier.padding(12.dp))

            errorMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                androidx.compose.foundation.layout.Spacer(Modifier.padding(8.dp))
            }

            OutlinedTextField(
                value = serverAddress,
                onValueChange = { serverAddress = it },
                label = { Text("Server address") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                singleLine = true,
            )
            androidx.compose.foundation.layout.Spacer(Modifier.padding(6.dp))
            OutlinedTextField(
                value = username,
                onValueChange = { username = it },
                label = { Text("Username") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            androidx.compose.foundation.layout.Spacer(Modifier.padding(6.dp))
            OutlinedTextField(
                value = password,
                onValueChange = { password = it },
                label = { Text("Password") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
            )
            androidx.compose.foundation.layout.Spacer(Modifier.padding(12.dp))

            Button(
                onClick = { onLogin(serverAddress.trim(), username.trim(), password) },
                enabled = !isLoggingIn && username.isNotBlank() && password.isNotEmpty(),
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (isLoggingIn) {
                    CircularProgressIndicator(modifier = Modifier.padding(2.dp))
                } else {
                    Text("Sign in")
                }
            }
        }
    }
}

/** Convenience default used by [com.onyx.MainActivity] so the field starts pre-filled, mirroring Dart's identical behavior. */
fun defaultServerAddressFor(prefs: SessionPreferences): String = prefs.serverAddress
    .ifBlank { SessionPreferences.DEFAULT_SERVER_ADDRESS }

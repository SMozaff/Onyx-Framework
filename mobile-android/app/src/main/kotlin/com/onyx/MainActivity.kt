package com.onyx

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.onyx.session.OnyxSessionViewModel
import com.onyx.session.OnyxUiState
import com.onyx.session.SessionPreferences
import com.onyx.ui.LoginScreen
import com.onyx.ui.StartupErrorScreen
import com.onyx.ui.defaultServerAddressFor

/**
 * A3's real startup entry point: renders whichever screen
 * [OnyxSessionViewModel.state] says is current (loading / needs-login /
 * ready / startup-error), mirroring `main.dart::restartApp()`'s
 * branching precisely rather than assuming a single static screen the
 * way A1's skeleton did.
 */
class MainActivity : ComponentActivity() {
    private val viewModel: OnyxSessionViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            OnyxRoot(viewModel)
        }
    }
}

@Composable
fun OnyxRoot(viewModel: OnyxSessionViewModel) {
    val state by viewModel.state.collectAsState()
    val loginError by viewModel.loginError.collectAsState()
    val context = androidx.compose.ui.platform.LocalContext.current

    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            when (val current = state) {
                is OnyxUiState.Loading -> LoadingScreen()
                is OnyxUiState.NeedsLogin -> LoginScreen(
                    defaultServerAddress = defaultServerAddressFor(SessionPreferences(context)),
                    isLoggingIn = false,
                    errorMessage = loginError,
                    onLogin = { serverAddress, username, password ->
                        viewModel.login(serverAddress, username, password)
                    },
                )
                is OnyxUiState.StartupError -> StartupErrorScreen(
                    message = current.message,
                    technicalDetail = current.technicalDetail,
                    onRetry = viewModel::retry,
                    onSignOutAndRetry = viewModel::signOutAndRetry,
                )
                is OnyxUiState.Ready -> ReadyScreen(
                    username = current.username,
                    onSignOut = viewModel::signOutAndRetry,
                )
            }
        }
    }
}

@Composable
private fun LoadingScreen() {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
    ) {
        CircularProgressIndicator()
    }
}

/**
 * Placeholder post-login content -- real screens (Dashboard, Missions,
 * Tasks, ...) are A4's scope, not this one. Shows enough to prove the
 * full login -> mobile-core -> session flow actually works end to end.
 */
@Composable
private fun ReadyScreen(username: String, onSignOut: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
    ) {
        Text("Signed in as $username")
        Text("ONYX Android (A3 skeleton -- real screens are A4)")
        Button(onClick = onSignOut) {
            Text("Sign out")
        }
    }
}

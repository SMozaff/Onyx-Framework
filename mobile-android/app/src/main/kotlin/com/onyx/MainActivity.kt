package com.onyx

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.onyx.controller.OnyxController
import com.onyx.session.OnyxSessionViewModel
import com.onyx.session.OnyxUiState
import com.onyx.session.SessionPreferences
import com.onyx.ui.AppShell
import com.onyx.ui.LoginScreen
import com.onyx.ui.StartupErrorScreen
import com.onyx.ui.defaultServerAddressFor

/**
 * The real startup entry point: renders whichever screen
 * [OnyxSessionViewModel.state] says is current (loading / needs-login /
 * ready / startup-error), mirroring `main.dart::restartApp()`'s
 * branching precisely. [OnyxUiState.Ready] now (A4) hands off to the
 * real app shell ([AppShell]) instead of a placeholder screen.
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
                is OnyxUiState.Ready -> {
                    // Keyed on the native handle so a sign-out (which
                    // frees that handle) followed by a fresh login
                    // (which mints a new one) gets a brand-new
                    // OnyxController rather than one still holding a
                    // freed/stale handle -- Compose's viewModel(key=...)
                    // creates (and eventually discards) a distinct
                    // ViewModel instance per key, exactly this case.
                    val controller: OnyxController = viewModel(
                        key = current.handle.toString(),
                        factory = OnyxController.Factory(current.handle, current.organizationId, current.userId),
                    )
                    AppShell(controller = controller)
                }
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

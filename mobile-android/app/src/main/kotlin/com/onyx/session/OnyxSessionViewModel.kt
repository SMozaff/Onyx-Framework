package com.onyx.session

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.onyx.bridge.MobileCoreBridge
import com.onyx.net.AuthApi
import com.onyx.net.MobileAccessRestrictedException
import com.onyx.security.SecureTokenStore
import com.onyx.util.UuidCodec
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File

private const val TAG = "OnyxSessionViewModel"

/**
 * The real startup state machine for A3, mirroring `main.dart`'s
 * `restartApp()` precisely: no saved, real session -> [NeedsLogin]; a
 * saved session -> open `mobile-core` under the real, previously
 * logged-in identity and go [Ready]; any failure along the way ->
 * [StartupError], never a silently blank/crashed screen.
 *
 * # No manual identity entry, ever (carried forward deliberately, not independently rediscovered)
 * Dart's own history (`startup_error_screen.dart`'s doc comment) records
 * a real, already-fixed security hole: a startup-failure recovery
 * screen used to let anyone type in an arbitrary `organization_id`/
 * `user_id` by hand, with zero authentication. This state machine's
 * only two recovery actions from [StartupError] are `retry()` (try the
 * exact same startup sequence again) and `signOutAndRetry()` (clear the
 * saved identity/session entirely and fall back to a real login) --
 * there is no state, screen, or method anywhere in this class that
 * accepts a caller-supplied organization/user id. This is the same
 * property Dart's own fixed history establishes, deliberately
 * preserved here from day one rather than something this task
 * independently rediscovered.
 */
sealed interface OnyxUiState {
    data object Loading : OnyxUiState
    data object NeedsLogin : OnyxUiState

    /**
     * `handle`/`userId` were added for A4: [com.onyx.controller.OnyxController]
     * (the shared-refresh, single-source-of-truth ViewModel every real
     * screen reads from -- Kotlin's port of `ui/app.dart`'s
     * `OnyxController`) needs both the live native handle and the
     * caller's own user id to build real command envelopes
     * ([com.onyx.model.CommandEnvelopeFactory]), exactly mirroring how
     * Dart's `main.dart::initializeFfiMobileCore` sets
     * `api.envelopeFactory` right after construction.
     */
    data class Ready(val username: String, val organizationId: String, val userId: String, val handle: Long) : OnyxUiState
    data class StartupError(val message: String, val technicalDetail: String) : OnyxUiState
}

class OnyxSessionViewModel(application: Application) : AndroidViewModel(application) {
    private val sessionPrefs = SessionPreferences(application)
    private val tokenStore = SecureTokenStore(application)

    private val _state = MutableStateFlow<OnyxUiState>(OnyxUiState.Loading)
    val state: StateFlow<OnyxUiState> = _state.asStateFlow()

    private val _loginError = MutableStateFlow<String?>(null)
    val loginError: StateFlow<String?> = _loginError.asStateFlow()

    private var nativeHandle: Long = 0
    private var refreshJob: Job? = null

    init {
        viewModelScope.launch { startup() }
    }

    override fun onCleared() {
        refreshJob?.cancel()
        freeNativeHandleIfAny()
        super.onCleared()
    }

    private fun freeNativeHandleIfAny() {
        if (nativeHandle != 0L) {
            MobileCoreBridge.nativeFree(nativeHandle)
            nativeHandle = 0
        }
    }

    /** Mirrors `main.dart::restartApp()`'s ffi-mode branch (this skeleton has no HTTP-mode). */
    private suspend fun startup() {
        _state.value = OnyxUiState.Loading
        if (!sessionPrefs.hasRealSession) {
            _state.value = OnyxUiState.NeedsLogin
            return
        }
        val organizationId = sessionPrefs.organizationId
        val userId = sessionPrefs.userId
        val username = sessionPrefs.username
        if (organizationId == null || userId == null || username == null) {
            // hasRealSession set but the identity fields are missing --
            // an inconsistent state that must never happen from a real
            // login, but treated as "no session" (fail-safe) rather
            // than crashing on a null, matching this project's general
            // "a corrupted saved value is an error state, not a crash"
            // posture (mirrors startup_error_screen.dart's own framing).
            _state.value = OnyxUiState.NeedsLogin
            return
        }
        openMobileCoreAndGoReady(organizationId, userId, username)
    }

    private suspend fun openMobileCoreAndGoReady(organizationId: String, userId: String, username: String) {
        try {
            freeNativeHandleIfAny()
            val dbPath = File(getApplication<Application>().filesDir, "onyx.sqlite").absolutePath
            val configJson = JSONObject()
                .put("organization_id", org.json.JSONArray(UuidCodec.uuidToBytes(organizationId)))
                .put("cloud_relay_endpoint", sessionPrefs.relayEndpoint)
                .toString()
            val handle = MobileCoreBridge.nativeNew(dbPath, configJson)
            if (handle == 0L) {
                throw IllegalStateException("mobile_core_new failed (invalid config, or local database/migration setup failed)")
            }
            nativeHandle = handle

            // Best-effort hierarchy population, same "logged, not
            // propagated as a startup failure" handling as
            // `desktop-shell::login`/Dart's `refreshHierarchyBestEffort`
            // -- a transient network failure here must not lock someone
            // out of an app that would otherwise work fully offline.
            val accessToken = tokenStore.readAccessToken()
            if (accessToken != null) {
                try {
                    val hierarchyJson = AuthApi(sessionPrefs.serverAddress).fetchHierarchyJson(accessToken)
                    MobileCoreBridge.nativeSetHierarchy(handle, hierarchyJson)
                } catch (error: Exception) {
                    Log.w(TAG, "Best-effort hierarchy refresh failed at startup", error)
                }
            }

            scheduleProactiveTokenRefresh()
            _state.value = OnyxUiState.Ready(username = username, organizationId = organizationId, userId = userId, handle = handle)
        } catch (error: Exception) {
            Log.e(TAG, "Startup failed", error)
            _state.value = OnyxUiState.StartupError(
                message = friendlyStartupError(error),
                technicalDetail = error.stackTraceToString(),
            )
        }
    }

    /**
     * Real login, mirroring `ffi_login_screen.dart`'s `_login()` exactly
     * in sequence: authenticate -> persist tokens to secure storage
     * (before the non-secret flag, so a crash between the two writes
     * never leaves `hasRealSession` set with no token to back it) ->
     * persist non-secret identity -> open mobile-core -> best-effort
     * hierarchy -> Ready.
     */
    fun login(serverAddress: String, username: String, password: String) {
        viewModelScope.launch {
            _loginError.value = null
            try {
                val authApi = AuthApi(serverAddress)
                val result = authApi.login(username, password)

                tokenStore.save(accessToken = result.accessToken, refreshToken = result.refreshToken)
                sessionPrefs.serverAddress = serverAddress
                sessionPrefs.username = result.username
                sessionPrefs.organizationId = result.organizationId
                sessionPrefs.userId = result.userId
                sessionPrefs.hasRealSession = true

                openMobileCoreAndGoReady(result.organizationId, result.userId, result.username)
            } catch (error: Exception) {
                Log.w(TAG, "Login failed", error)
                _loginError.value = friendlyLoginError(error)
            }
        }
    }

    /** Re-runs the exact same startup sequence -- the "Retry" action from [OnyxUiState.StartupError]. */
    fun retry() {
        viewModelScope.launch { startup() }
    }

    /**
     * Clears the saved identity/session entirely and returns to
     * [OnyxUiState.NeedsLogin] -- the *only* recovery path this class
     * offers for a bad identity/session, per this class's own top-level
     * doc comment on why manual identity entry was never built here.
     */
    fun signOutAndRetry() {
        viewModelScope.launch {
            refreshJob?.cancel()
            freeNativeHandleIfAny()
            val refreshToken = tokenStore.readRefreshToken()
            if (refreshToken != null) {
                try {
                    AuthApi(sessionPrefs.serverAddress).logout(refreshToken)
                } catch (_: Exception) {
                    // Best-effort, mirrors AuthApi.logout's own
                    // fail-open-locally reasoning -- local state is
                    // still cleared below regardless.
                }
            }
            tokenStore.clear()
            sessionPrefs.clear()
            _state.value = OnyxUiState.NeedsLogin
        }
    }

    /**
     * Proactively renews the access token before its `expires_in` TTL
     * lapses, mirroring `OnyxController.initialize`'s periodic timer
     * (`ui/app.dart`) and `main.dart::refreshHierarchyBestEffort`'s
     * reactive-refresh fallback combined into one mechanism: this is
     * the *primary* renewal path (proactive), not just a 401 retry,
     * since a session left open for the server's full 1-hour access
     * token TTL would otherwise sit on a stale token until something
     * else happened to fail first.
     */
    private fun scheduleProactiveTokenRefresh() {
        refreshJob?.cancel()
        refreshJob = viewModelScope.launch {
            while (true) {
                val refreshToken = tokenStore.readRefreshToken() ?: return@launch
                try {
                    val result = AuthApi(sessionPrefs.serverAddress).refresh(refreshToken)
                    tokenStore.save(accessToken = result.accessToken, refreshToken = result.refreshToken)
                    // Refresh at 80% of the token's real TTL, the same
                    // "well inside the TTL, not only reactively after a
                    // request already failed" reasoning Dart's own doc
                    // comment gives for its periodic interval choice.
                    delay(result.expiresInSeconds * 800L)
                } catch (error: Exception) {
                    // A refresh token that has itself expired (7 days)
                    // or was revoked requires a real password login
                    // again -- logged, not retried in a tight loop; the
                    // next login will reschedule this from scratch.
                    Log.w(TAG, "Proactive token refresh failed; will not retry until next login", error)
                    return@launch
                }
            }
        }
    }
}

private fun friendlyLoginError(error: Throwable): String = when {
    error is MobileAccessRestrictedException ->
        "Mobile access is not enabled for your user class in this organization. Ask an admin to enable it in Settings."
    error.toString().let { it.contains("Unable to resolve host") || it.contains("ConnectException") || it.contains("SocketTimeoutException") } ->
        "Could not reach the server. Check that api-server is running and the address above is correct, " +
            "and that this device is on the same Wi-Fi network as the server."
    error.toString().contains("401") || error.toString().contains("INVALID_CREDENTIALS") ->
        "Invalid username or password."
    else -> "Sign-in failed: ${error.message ?: error}"
}

private fun friendlyStartupError(error: Throwable): String = when {
    error.message?.contains("mobile_core_new failed") == true ->
        "The local database or sync engine could not be initialized. This can happen after a corrupted " +
            "update or if storage is full. Try \"Sign out and retry\" below, or check available device storage."
    else -> "An unexpected error occurred while starting the app."
}

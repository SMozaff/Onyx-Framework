package com.onyx.session

import android.content.Context

/**
 * Non-secret session identity, mirroring Dart's own split exactly
 * (`main.dart`/`ffi_login_screen.dart`): `organization_id`/`user_id`/
 * `username`/`server_address` and the "has a real, logged-in session"
 * flag live in plain `SharedPreferences` (appropriate for non-sensitive
 * values), while the actual bearer tokens live only in
 * [com.onyx.security.SecureTokenStore]. [hasRealSession] mirrors Dart's
 * `hasRealFfiSessionKey` precisely: it is set only after a real
 * `POST /api/auth/login` succeeds, and its absence is what routes
 * startup to the login screen instead of silently opening mobile-core
 * under a stale or placeholder identity.
 */
class SessionPreferences(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)

    var organizationId: String?
        get() = prefs.getString(KEY_ORGANIZATION_ID, null)
        set(value) = prefs.edit().putString(KEY_ORGANIZATION_ID, value).apply()

    var userId: String?
        get() = prefs.getString(KEY_USER_ID, null)
        set(value) = prefs.edit().putString(KEY_USER_ID, value).apply()

    var username: String?
        get() = prefs.getString(KEY_USERNAME, null)
        set(value) = prefs.edit().putString(KEY_USERNAME, value).apply()

    var serverAddress: String
        get() = prefs.getString(KEY_SERVER_ADDRESS, DEFAULT_SERVER_ADDRESS) ?: DEFAULT_SERVER_ADDRESS
        set(value) = prefs.edit().putString(KEY_SERVER_ADDRESS, value).apply()

    var hasRealSession: Boolean
        get() = prefs.getBoolean(KEY_HAS_REAL_SESSION, false)
        set(value) = prefs.edit().putBoolean(KEY_HAS_REAL_SESSION, value).apply()

    /**
     * Clears every field this class owns. Called only from the
     * sign-out/recovery path -- never leaves a partial identity behind
     * that a later `hasRealSession` check could misread as valid, per
     * this project's no-manual-identity-entry invariant (see
     * `com.onyx.ui.StartupErrorScreen`'s doc comment for the security
     * property this preserves from Dart's own fixed history).
     */
    fun clear() {
        prefs.edit()
            .remove(KEY_ORGANIZATION_ID)
            .remove(KEY_USER_ID)
            .remove(KEY_USERNAME)
            .putBoolean(KEY_HAS_REAL_SESSION, false)
            .apply()
    }

    companion object {
        private const val PREFS_FILE = "onyx_session_prefs"
        private const val KEY_ORGANIZATION_ID = "organization_id"
        private const val KEY_USER_ID = "user_id"
        private const val KEY_USERNAME = "username"
        private const val KEY_SERVER_ADDRESS = "server_address"
        private const val KEY_HAS_REAL_SESSION = "has_real_session"
        const val DEFAULT_SERVER_ADDRESS = "http://192.168.1.1:3000"
    }
}

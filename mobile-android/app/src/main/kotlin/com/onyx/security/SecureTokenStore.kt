package com.onyx.security

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Secure, on-device persistence for the real login session (access
 * token / refresh token), Kotlin's equivalent of Dart's
 * `FfiSessionStorage` (`mobile/lib/net/session_storage.dart`) --
 * deliberately not the plain, unencrypted `SharedPreferences` this app
 * also uses for non-secret values (org/user id, username, server
 * address -- see [com.onyx.session.SessionPreferences]).
 *
 * # Real correction from Dart's own approach -- checked, not assumed current
 * Dart's reference uses `flutter_secure_storage`, which wraps Android's
 * `EncryptedSharedPreferences` (`androidx.security:security-crypto`).
 * Checked directly against that library's own release notes before
 * writing this (not assumed still current just because Dart's side
 * uses it): as of version 1.1.0-beta01, **all APIs in
 * `androidx.security:security-crypto` -- including
 * `EncryptedSharedPreferences` -- are deprecated "in favour of existing
 * platform APIs and direct use of Android Keystore."** Kotlin's native
 * side therefore does NOT mirror Dart's exact library choice here; it
 * implements the now-recommended pattern directly: an AES-256-GCM key
 * generated and held inside the Android Keystore (`KeyGenParameterSpec`,
 * hardware-backed where the device supports it, never leaves the
 * secure element/TEE in exportable form), used to encrypt the token
 * bytes, with only the resulting ciphertext + IV persisted in a plain
 * `SharedPreferences` file. This satisfies the same real requirement
 * Dart's doc comment states (a real bearer token needs materially more
 * protection than a placeholder UUID) via the platform's own
 * currently-recommended mechanism rather than a now-deprecated wrapper
 * library.
 */
class SecureTokenStore(context: Context) {
    private val appContext = context.applicationContext
    private val prefs = appContext.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)

    private val keyStore: KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    private fun getOrCreateKey(): SecretKey {
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val spec = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build()
        keyGenerator.init(spec)
        return keyGenerator.generateKey()
    }

    private fun encrypt(plainText: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val ciphertext = cipher.doFinal(plainText.toByteArray(Charsets.UTF_8))
        // IV is not secret -- prepended so decrypt() can recover it
        // without a second stored field, the standard GCM pattern.
        val combined = cipher.iv + ciphertext
        return Base64.encodeToString(combined, Base64.NO_WRAP)
    }

    private fun decrypt(stored: String): String? {
        return try {
            val combined = Base64.decode(stored, Base64.NO_WRAP)
            val iv = combined.copyOfRange(0, GCM_IV_LENGTH_BYTES)
            val ciphertext = combined.copyOfRange(GCM_IV_LENGTH_BYTES, combined.size)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(GCM_TAG_LENGTH_BITS, iv))
            String(cipher.doFinal(ciphertext), Charsets.UTF_8)
        } catch (_: Exception) {
            // A corrupted/tampered value or an unwrapped key (e.g. after
            // the app's data was restored to a different device, which
            // Keystore keys never survive) must not crash session
            // startup -- treated as "no stored token", the same
            // fail-safe direction as a missing value, per this
            // project's login/session invariant that only a real login
            // can ever produce a usable session.
            null
        }
    }

    fun save(accessToken: String, refreshToken: String) {
        prefs.edit()
            .putString(KEY_ACCESS_TOKEN, encrypt(accessToken))
            .putString(KEY_REFRESH_TOKEN, encrypt(refreshToken))
            .apply()
    }

    fun readAccessToken(): String? = prefs.getString(KEY_ACCESS_TOKEN, null)?.let(::decrypt)

    fun readRefreshToken(): String? = prefs.getString(KEY_REFRESH_TOKEN, null)?.let(::decrypt)

    fun clear() {
        prefs.edit().remove(KEY_ACCESS_TOKEN).remove(KEY_REFRESH_TOKEN).apply()
        try {
            keyStore.deleteEntry(KEY_ALIAS)
        } catch (_: Exception) {
            // Best-effort key rotation on sign-out; a failure here does
            // not leave any token recoverable, since the ciphertext
            // fields above are already cleared.
        }
    }

    companion object {
        private const val PREFS_FILE = "onyx_secure_session"
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val KEY_ALIAS = "onyx_session_token_key"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_IV_LENGTH_BYTES = 12
        private const val GCM_TAG_LENGTH_BITS = 128
        private const val KEY_ACCESS_TOKEN = "access_token"
        private const val KEY_REFRESH_TOKEN = "refresh_token"
    }
}

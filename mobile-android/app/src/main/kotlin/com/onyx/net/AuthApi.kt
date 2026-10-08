package com.onyx.net

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit

private val JSON_MEDIA_TYPE = "application/json".toMediaType()

/**
 * Thrown instead of a generic failure when the server's login rejection
 * is specifically `MOBILE_ACCESS_RESTRICTED` -- exact Kotlin mirror of
 * Dart's `MobileAccessRestrictedException` (`mobile/lib/net/auth.dart`),
 * same reasoning: every other credential failure deliberately comes
 * back as the generic `INVALID_CREDENTIALS` (per `auth.rs`'s own
 * audit-finding H-01 doc comment), so only this one case gets a
 * distinct, more specific message.
 */
class MobileAccessRestrictedException : Exception("Mobile access is not enabled for this user's class")

class LoginResult(
    val accessToken: String,
    val refreshToken: String,
    val expiresInSeconds: Long,
    val userId: String,
    val organizationId: String,
    val username: String,
)

class RefreshResult(val accessToken: String, val refreshToken: String, val expiresInSeconds: Long)

/**
 * Login/hierarchy/refresh against `api-server`'s real routes, Kotlin's
 * equivalent of Dart's `OnyxHttpAuthApi` (`mobile/lib/net/auth.dart`).
 * Request/response field names match `crates/bins/api-server/src/
 * routes/auth.rs`'s `LoginRequest`/`LoginResponse` structs exactly
 * (confirmed by reading that file directly for A3, the same way Dart's
 * own doc comment states it did) -- `username`/`password`/`client_type`
 * in, `access_token`/`refresh_token`/`expires_in`/`user` out, `user` =
 * `{id, username, organization_id}`.
 *
 * Per A3's own architectural constraint (confirmed in `mobile-core`'s
 * own source: "mobile has no login/auth" in Rust): this is a plain
 * HTTP client, not a JNI call. Login stays entirely outside
 * `mobile-android-jni`/`mobile-core` -- only the *result* (organization
 * id, user id, hierarchy JSON) is later handed into `mobile-core` via
 * `MobileCoreBridge.nativeNew`/`nativeSetHierarchy`, exactly mirroring
 * Dart's `ffi_login_screen.dart` sequencing.
 */
class AuthApi(private val serverAddress: String) {
    private val client = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(15, TimeUnit.SECONDS)
        .build()

    private fun url(path: String) = serverAddress.trimEnd('/') + path

    /**
     * Throws [MobileAccessRestrictedException] for that specific server
     * rejection, [IOException] for a network failure, or
     * [AuthApiException] for any other non-2xx response (mirrors Dart's
     * "rethrow the raw failure for every other case" -- there is no more
     * specific error to surface for a generic credential failure than
     * "login failed", per `auth.rs`'s own deliberate error-uniformity
     * design).
     */
    suspend fun login(username: String, password: String): LoginResult = withContext(Dispatchers.IO) {
        val body = JSONObject()
            .put("username", username)
            .put("password", password)
            .put("client_type", "mobile")
            .toString()
            .toRequestBody(JSON_MEDIA_TYPE)
        val request = Request.Builder().url(url("/api/auth/login")).post(body).build()

        client.newCall(request).execute().use { response ->
            val responseBody = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                val code = runCatching { JSONObject(responseBody).getJSONObject("error").getString("code") }
                    .getOrNull()
                if (code == "MOBILE_ACCESS_RESTRICTED") throw MobileAccessRestrictedException()
                throw AuthApiException(response.code, responseBody)
            }
            val json = JSONObject(responseBody)
            val user = json.getJSONObject("user")
            LoginResult(
                accessToken = json.getString("access_token"),
                refreshToken = json.getString("refresh_token"),
                expiresInSeconds = json.getLong("expires_in"),
                userId = user.getString("id"),
                organizationId = user.getString("organization_id"),
                username = user.getString("username"),
            )
        }
    }

    /**
     * Fetches the org's reporting-line tree from
     * `GET /api/users/hierarchy` and returns it as a raw JSON string --
     * the exact shape `MobileCoreBridge.nativeSetHierarchy` expects.
     * Requires [accessToken] from a successful [login] (or a stored,
     * still-valid session).
     */
    suspend fun fetchHierarchyJson(accessToken: String): String = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url(url("/api/users/hierarchy"))
            .header("Authorization", "Bearer $accessToken")
            .get()
            .build()
        client.newCall(request).execute().use { response ->
            val responseBody = response.body?.string().orEmpty()
            if (!response.isSuccessful) throw AuthApiException(response.code, responseBody)
            // The server already returns a JSON array; re-serializing
            // through JSONArray (rather than passing responseBody
            // straight through) validates it really is well-formed JSON
            // before it's handed to the native layer.
            JSONArray(responseBody).toString()
        }
    }

    /**
     * Redeems [refreshToken] via `POST /api/auth/refresh` for a new
     * access token. The server rotates the refresh token on every use
     * (see `auth::refresh`'s own doc comment) -- callers must persist
     * the returned [RefreshResult.refreshToken], not reuse the old one.
     */
    suspend fun refresh(refreshToken: String): RefreshResult = withContext(Dispatchers.IO) {
        val body = JSONObject().put("refresh_token", refreshToken).toString().toRequestBody(JSON_MEDIA_TYPE)
        val request = Request.Builder().url(url("/api/auth/refresh")).post(body).build()
        client.newCall(request).execute().use { response ->
            val responseBody = response.body?.string().orEmpty()
            if (!response.isSuccessful) throw AuthApiException(response.code, responseBody)
            val json = JSONObject(responseBody)
            RefreshResult(
                accessToken = json.getString("access_token"),
                refreshToken = json.getString("refresh_token"),
                expiresInSeconds = json.getLong("expires_in"),
            )
        }
    }

    /**
     * Best-effort, mirroring Dart's `OnyxHttpAuthApi.logout`: the local
     * session is always cleared by the caller regardless of whether
     * this call succeeds, since a failed logout call must never trap
     * someone in a logged-in-looking state on their own device.
     */
    suspend fun logout(refreshToken: String) = withContext(Dispatchers.IO) {
        try {
            val body = JSONObject().put("refresh_token", refreshToken).toString().toRequestBody(JSON_MEDIA_TYPE)
            val request = Request.Builder().url(url("/api/auth/logout")).post(body).build()
            client.newCall(request).execute().close()
        } catch (_: Exception) {
            // Server-side revocation failed or was unreachable -- the
            // caller still clears local state. A stale-but-unrevoked
            // token is an existing, separately-tracked server-side gap
            // (this project's docs/AUDIT_REGISTER.md finding H-02), not
            // something this client can fix by retrying harder.
        }
    }
}

class AuthApiException(val httpStatus: Int, val rawBody: String) :
    Exception("HTTP $httpStatus: $rawBody")

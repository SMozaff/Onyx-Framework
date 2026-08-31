package com.onyx.model

import com.onyx.util.UuidCodec
import org.json.JSONObject

/**
 * One loaded aggregate (mission/task/notification/...), Kotlin's exact
 * mirror of Dart's `LoadedAggregate` (`mobile/lib/bridge/bridge.dart`):
 * `id` is a raw 16-byte array (mobile-core's own `ObjectId` shape, not a
 * UUID string), `aggregate` is the raw domain payload, `title`/`status`/
 * `description` are derived from `aggregate` with the exact same
 * fallback chain Dart uses (`name` then `title` then `"Untitled"` for
 * the title; `status` then `"Unknown"`).
 */
class LoadedAggregate(
    val rawId: IntArray,
    val aggregate: JSONObject,
    val version: Long,
    val lifecycleEpoch: Long,
    val authorityEpoch: Long,
    val updatedAt: Long,
) {
    val id: String get() = UuidCodec.bytesToUuid(rawId)
    val title: String get() = stringOrNull("name") ?: stringOrNull("title") ?: "Untitled"
    val status: String get() = stringOrNull("status") ?: "Unknown"
    val description: String? get() = stringOrNull("description")

    private fun stringOrNull(key: String): String? =
        if (aggregate.has(key) && !aggregate.isNull(key)) aggregate.getString(key) else null

    companion object {
        fun fromJson(json: JSONObject): LoadedAggregate {
            val idArray = json.optJSONArray("id")
            val rawId = IntArray(idArray?.length() ?: 0) { idArray!!.getInt(it) }
            return LoadedAggregate(
                rawId = rawId,
                aggregate = json.optJSONObject("aggregate") ?: JSONObject(),
                version = json.optLong("version", 0),
                lifecycleEpoch = json.optLong("lifecycle_epoch", 0),
                authorityEpoch = json.optLong("authority_epoch", 0),
                updatedAt = json.optLong("updated_at", 0),
            )
        }
    }
}

/** Kotlin mirror of Dart's `SyncSnapshot`. */
data class SyncSnapshot(
    val online: Boolean,
    val pendingOutboxCount: Int,
    val openConflictCount: Int,
    val lastSyncAttempt: String?,
    val lastSyncResult: String?,
) {
    companion object {
        val EMPTY = SyncSnapshot(online = false, pendingOutboxCount = 0, openConflictCount = 0, lastSyncAttempt = null, lastSyncResult = null)

        fun fromJson(json: JSONObject): SyncSnapshot = SyncSnapshot(
            online = json.optBoolean("online", false),
            pendingOutboxCount = json.optInt("pending_outbox_count", 0),
            openConflictCount = json.optInt("open_conflict_count", 0),
            lastSyncAttempt = if (json.has("last_sync_attempt") && !json.isNull("last_sync_attempt")) json.getString("last_sync_attempt") else null,
            lastSyncResult = if (json.has("last_sync_result") && !json.isNull("last_sync_result")) json.getString("last_sync_result") else null,
        )
    }
}

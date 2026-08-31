package com.onyx.model

import org.json.JSONObject

/**
 * A real, open synchronization conflict, Kotlin's port of Dart's
 * `SyncConflict` (`mobile/lib/bridge/bridge.dart`). Keeps the raw JSON
 * object exactly as `mobile_core_list_conflicts` returned it -- needed
 * unchanged by [com.onyx.controller.OnyxController.resolveConflict] as
 * `mobile_core_resolve_conflict`'s own `conflict_json` argument, per
 * that function's doc comment ("must contain the serialized
 * `conflict_id` field returned by `mobile_core_list_conflicts`") -- so
 * this class re-derives display fields lazily from [raw] rather than
 * copying them out and losing the rest of the payload `resolveConflict`
 * still needs to send back.
 */
class SyncConflict(val raw: JSONObject) {
    val fieldPath: String
        get() = if (raw.has("field_path") && !raw.isNull("field_path")) raw.getString("field_path") else "unknown"

    val localValue: Any?
        get() = payloadValue(raw.opt("local_operation"))

    val remoteValue: Any?
        get() = payloadValue(raw.opt("remote_operation"))

    private fun payloadValue(operation: Any?): Any? {
        if (operation !is JSONObject) return operation
        val payload = operation.opt("payload")
        if (payload is JSONObject && payload.has("value")) return payload.opt("value")
        return payload
    }

    companion object {
        fun fromJson(json: JSONObject): SyncConflict = SyncConflict(json)
    }
}

/** Kotlin's port of Dart's `ConflictChoice` enum -- `name.lowercase()` matches `mobile_core_resolve_conflict`'s real, current string match exactly (`"local"`/`"remote"`/`"escalate"`). */
enum class ConflictChoice(val wireValue: String) {
    LOCAL("local"),
    REMOTE("remote"),
    ESCALATE("escalate"),
}

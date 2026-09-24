package com.onyx.model

import com.onyx.util.UuidCodec
import org.json.JSONArray
import org.json.JSONObject

/**
 * Builds the same real query envelope shape `mobile-core`'s
 * `mobile_core_execute_query` consumes (`crates/mobile-core/src/
 * ffi_queries.rs`) and `app_state.rs`'s `QueryRegistry` dispatches on:
 * `{"query_type": "GetMission"|"GetTask", "target_id": <16 bytes>}` —
 * a `QueryEnvelope` with no organization field (the registry is bound to
 * the app's own organization at `mobile_core_new`, so `target_id`, like
 * every `ObjectId` in this FFI interface, is a raw 16-byte array, not a
 * UUID string).
 *
 * Unlike [CommandEnvelopeFactory] this factory is stateless: it needs no
 * organization/user context, which is exactly why the query path could be
 * wired as a one-shot counterpart to the (already-built) command path.
 */
class QueryEnvelopeFactory {
    fun create(queryType: String, targetId: String): JSONObject =
        JSONObject()
            .put("query_type", queryType)
            .put("target_id", JSONArray(UuidCodec.uuidToBytes(targetId)))

    fun getMission(targetId: String): JSONObject = create("GetMission", targetId)

    fun getTask(targetId: String): JSONObject = create("GetTask", targetId)
}
package com.onyx.model

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Real parsing tests against Dart's exact `SyncConflict` field
 * derivation (`mobile/lib/bridge/bridge.dart`), confirmed by reading
 * that class directly for A5 -- not assumed from `mobile_core_list_
 * conflicts`'s Rust struct shape alone.
 */
class SyncConflictTest {
    @Test
    fun `fieldPath falls back to unknown when absent`() {
        val withField = SyncConflict.fromJson(JSONObject().put("field_path", "status"))
        assertEquals("status", withField.fieldPath)

        val without = SyncConflict.fromJson(JSONObject())
        assertEquals("unknown", without.fieldPath)
    }

    @Test
    fun `localValue and remoteValue unwrap the operation's payload value`() {
        val conflict = SyncConflict.fromJson(
            JSONObject(
                """
                {
                  "conflict_id": "11111111-1111-4111-8111-111111111111",
                  "field_path": "status",
                  "local_operation": {"payload": {"value": "Approved"}},
                  "remote_operation": {"payload": {"value": "Rejected"}}
                }
                """.trimIndent(),
            ),
        )
        assertEquals("Approved", conflict.localValue)
        assertEquals("Rejected", conflict.remoteValue)
    }

    @Test
    fun `payload without a value key falls back to the whole payload object`() {
        val conflict = SyncConflict.fromJson(
            JSONObject(
                """{"local_operation": {"payload": {"name": "Recon Alpha"}}}""",
            ),
        )
        val local = conflict.localValue as JSONObject
        assertEquals("Recon Alpha", local.getString("name"))
    }

    @Test
    fun `a non-object operation is returned as-is, and a missing one is null`() {
        val withString = SyncConflict.fromJson(JSONObject().put("local_operation", "raw"))
        assertEquals("raw", withString.localValue)

        val missing = SyncConflict.fromJson(JSONObject())
        assertNull(missing.remoteValue)
    }

    @Test
    fun `raw keeps the full JSON needed to round-trip into resolveConflict`() {
        val json = JSONObject().put("conflict_id", "abc").put("field_path", "status")
        val conflict = SyncConflict.fromJson(json)
        assertEquals("abc", conflict.raw.getString("conflict_id"))
    }
}

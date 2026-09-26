package com.onyx.model

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Real parsing tests against the exact JSON shape `mobile-core`'s
 * `mobile_core_list_aggregates` returns (confirmed directly against
 * Dart's `LoadedAggregate.fromJson`, `mobile/lib/bridge/bridge.dart`,
 * for A4 -- not assumed from the Rust `LoadedAggregate` type name
 * alone).
 */
class LoadedAggregateTest {
    @Test
    fun `title falls back from name to title to Untitled, matching Dart's real fallback chain`() {
        val named = LoadedAggregate.fromJson(aggregateJson("""{"name": "Recon Alpha", "status": "Draft"}"""))
        assertEquals("Recon Alpha", named.title)

        val titled = LoadedAggregate.fromJson(aggregateJson("""{"title": "Patrol Bravo", "status": "Submitted"}"""))
        assertEquals("Patrol Bravo", titled.title)

        val neither = LoadedAggregate.fromJson(aggregateJson("""{"status": "Draft"}"""))
        assertEquals("Untitled", neither.title)
    }

    @Test
    fun `status falls back to Unknown when absent`() {
        val withStatus = LoadedAggregate.fromJson(aggregateJson("""{"name": "X", "status": "AwaitingApproval"}"""))
        assertEquals("AwaitingApproval", withStatus.status)

        val withoutStatus = LoadedAggregate.fromJson(aggregateJson("""{"name": "X"}"""))
        assertEquals("Unknown", withoutStatus.status)
    }

    @Test
    fun `description is null when absent or JSON null, not the string literal null`() {
        val absent = LoadedAggregate.fromJson(aggregateJson("""{"name": "X"}"""))
        assertNull(absent.description)

        val jsonNull = LoadedAggregate.fromJson(aggregateJson("""{"name": "X", "description": null}"""))
        assertNull(jsonNull.description)

        val present = LoadedAggregate.fromJson(aggregateJson("""{"name": "X", "description": "real text"}"""))
        assertEquals("real text", present.description)
    }

    @Test
    fun `id round-trips through the same 16-byte layout confirmed for the FFI config`() {
        val json = JSONObject(
            """{"id": [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15], "aggregate": {"name":"X"}, "version": 3, "lifecycle_epoch": 1, "authority_epoch": 2, "updated_at": 100}""",
        )
        val aggregate = LoadedAggregate.fromJson(json)
        assertEquals("00010203-0405-0607-0809-0a0b0c0d0e0f", aggregate.id)
        assertEquals(3L, aggregate.version)
        assertEquals(1L, aggregate.lifecycleEpoch)
        assertEquals(2L, aggregate.authorityEpoch)
    }

    private fun aggregateJson(aggregate: String): JSONObject = JSONObject(
        """{"id": [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15], "aggregate": $aggregate, "version": 0, "lifecycle_epoch": 0, "authority_epoch": 0, "updated_at": 0}""",
    )
}

class SyncSnapshotTest {
    @Test
    fun `fromJson matches Dart's real field names and null handling`() {
        val json = JSONObject(
            """{"online": true, "pending_outbox_count": 4, "open_conflict_count": 1, "last_sync_attempt": "2026-01-01T00:00:00Z", "last_sync_result": null}""",
        )
        val snapshot = SyncSnapshot.fromJson(json)
        assertEquals(true, snapshot.online)
        assertEquals(4, snapshot.pendingOutboxCount)
        assertEquals(1, snapshot.openConflictCount)
        assertEquals("2026-01-01T00:00:00Z", snapshot.lastSyncAttempt)
        assertNull(snapshot.lastSyncResult)
    }

    @Test
    fun `EMPTY matches Dart's SyncSnapshot#empty defaults`() {
        assertEquals(false, SyncSnapshot.EMPTY.online)
        assertEquals(0, SyncSnapshot.EMPTY.pendingOutboxCount)
        assertEquals(0, SyncSnapshot.EMPTY.openConflictCount)
    }
}

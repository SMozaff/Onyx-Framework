package com.onyx.model

import com.onyx.util.UuidCodec
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Confirms the envelope shape matches Dart's real
 * `CommandEnvelopeFactory.create` (`mobile/lib/bridge/bridge.dart`)
 * field-for-field, since `mobile_core_execute_command` deserializes
 * this JSON as `CommandEnvelope<Value>` -- a shape mismatch here would
 * fail every real command, not just this test.
 */
class CommandEnvelopeFactoryTest {
    private val organizationId = "11111111-1111-4111-8111-111111111111"
    private val userId = "22222222-3333-4444-8555-666666666666"
    private val factory = CommandEnvelopeFactory(organizationId, userId)

    @Test
    fun `target and actor carry organization_id and ids as 16-byte arrays, not UUID strings`() {
        val envelope = factory.create(
            commandType = "ApproveTask",
            targetType = "task",
            targetId = "33333333-4444-4444-8444-444444444444",
            payload = JSONObject().put("reason", "looks good"),
        )
        val target = envelope.getJSONObject("target")
        assertEquals(16, target.getJSONArray("id").length())
        assertEquals("task", target.getString("type"))
        assertEquals(16, target.getJSONArray("organization_id").length())

        val actor = envelope.getJSONObject("actor")
        assertEquals(16, actor.getJSONArray("user_id").length())
        assertEquals(16, actor.getJSONArray("device_id").length())

        // Dart's own real, current placeholder device id -- reproduced
        // exactly, not independently generated.
        assertEquals(
            UuidCodec.bytesToUuid(IntArray(16) { actor.getJSONArray("device_id").getInt(it) }),
            "22222222-2222-4222-8222-222222222222",
        )
    }

    @Test
    fun `authority_proof matches Dart's real local-first placeholder shape`() {
        val envelope = factory.create(
            commandType = "RejectTask",
            targetType = "task",
            targetId = "33333333-4444-4444-8444-444444444444",
            payload = JSONObject().put("reason", "missing evidence"),
        )
        val proof = envelope.getJSONObject("authority_proof")
        assertEquals("Jwt", proof.getString("proof_type"))
        assertTrue(proof.isNull("signature"))
        assertEquals(Long.MAX_VALUE, proof.getLong("expires_at"))
    }

    @Test
    fun `expected_version, lifecycle_epoch, and authority_epoch default to zero for a fresh aggregate`() {
        val envelope = factory.create(
            commandType = "CreateMission",
            targetType = "mission",
            targetId = "33333333-4444-4444-8444-444444444444",
            payload = JSONObject().put("name", "Test Mission"),
        )
        assertEquals(0L, envelope.getLong("expected_version"))
        assertEquals(0L, envelope.getLong("expected_lifecycle_epoch"))
        assertEquals(0L, envelope.getLong("expected_authority_epoch"))
    }

    @Test
    fun `optimistic-concurrency fields are threaded through from an already-loaded aggregate`() {
        val envelope = factory.create(
            commandType = "ActivateMission",
            targetType = "mission",
            targetId = "33333333-4444-4444-8444-444444444444",
            payload = JSONObject().put("reason", JSONObject.NULL),
            expectedVersion = 7,
            lifecycleEpoch = 2,
            authorityEpoch = 1,
        )
        assertEquals(7L, envelope.getLong("expected_version"))
        assertEquals(2L, envelope.getLong("expected_lifecycle_epoch"))
        assertEquals(1L, envelope.getLong("expected_authority_epoch"))
    }
}

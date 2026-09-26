package com.onyx.model

import com.onyx.util.UuidCodec
import org.json.JSONArray
import org.json.JSONObject

/**
 * Builds the same real command envelope shape Dart's
 * `CommandEnvelopeFactory` builds (`mobile/lib/bridge/bridge.dart`),
 * field-for-field -- confirmed by reading that class directly for A4,
 * not assumed from the general `CommandEnvelope<C>` Rust type shape
 * alone. Every id field is a raw 16-byte array (`UuidCodec.uuidToBytes`),
 * matching the same `ObjectId` derive `mobile-core`'s FFI layer expects
 * everywhere except the HTTP-facing DTOs.
 *
 * `deviceId` is a fixed literal (`"22222222-2222-4222-8222-222222222222"`),
 * copied exactly from Dart's own real, current value -- a known
 * placeholder in *this* codebase (no real per-device identity concept
 * exists yet at the command-envelope level for FFI-mode mobile), not
 * something this task independently invented or "fixed"; parity with
 * Dart's real behavior takes precedence here per A4's own instructions.
 * Likewise `authority_proof` carries `proof_type: "Jwt"` with a `null`
 * signature and a maximal `expires_at` -- Dart's own real, current
 * placeholder for local-first FFI mode, where the command dispatch path
 * does not verify a real JWT (confirmed by reading `bridge.dart`
 * directly), reproduced as-is rather than tightened unilaterally.
 */
class CommandEnvelopeFactory(val organizationId: String, val userId: String) {
    /**
     * Public (not just used internally by [create]) since A5's Files
     * screen needs it too: `mobile_core_upload_file` takes
     * organization/user/device id as plain UUID strings (parsed via
     * `OrganizationId`/`ObjectId`'s `FromStr`), unlike the raw-byte-array
     * shape every command-envelope id field above uses -- confirmed by
     * reading `ffi_files.rs` directly, matching Dart's own
     * `uploadFile`, which passes `envelopeFactory.organizationId`/
     * `userId`/`deviceId` straight through as UTF-8 strings.
     */
    val deviceId = "22222222-2222-4222-8222-222222222222"

    fun create(
        commandType: String,
        targetType: String,
        targetId: String,
        payload: JSONObject,
        expectedVersion: Long = 0,
        lifecycleEpoch: Long = 0,
        authorityEpoch: Long = 0,
    ): JSONObject {
        val nowNanos = System.currentTimeMillis() * 1_000_000L

        val target = JSONObject()
            .put("id", JSONArray(UuidCodec.uuidToBytes(targetId)))
            .put("type", targetType)
            .put("organization_id", JSONArray(UuidCodec.uuidToBytes(organizationId)))

        val actor = JSONObject()
            .put("user_id", JSONArray(UuidCodec.uuidToBytes(userId)))
            .put("device_id", JSONArray(UuidCodec.uuidToBytes(deviceId)))
            .put("organization_id", JSONArray(UuidCodec.uuidToBytes(organizationId)))

        val scope = JSONObject()
            .put("organization_id", JSONArray(UuidCodec.uuidToBytes(organizationId)))
            .put("object_type", targetType)
            .put("object_id", JSONObject.NULL)
            .put("command_types", JSONArray(listOf(commandType)))
            .put("delegation_depth", 0)

        val authorityProof = JSONObject()
            .put("proof_type", "Jwt")
            .put("scope", scope)
            .put("issued_at", 0)
            .put("expires_at", Long.MAX_VALUE)
            .put("signature", JSONObject.NULL)

        return JSONObject()
            .put("command_id", JSONArray(UuidCodec.uuidToBytes(UuidCodec.randomUuid())))
            .put("operation_id", JSONArray(UuidCodec.uuidToBytes(UuidCodec.randomUuid())))
            .put("command_type", commandType)
            .put("schema_version", "1.0.0")
            .put("target", target)
            .put("expected_version", expectedVersion)
            .put("expected_lifecycle_epoch", lifecycleEpoch)
            .put("expected_authority_epoch", authorityEpoch)
            .put("actor", actor)
            .put("authority_proof", authorityProof)
            .put("issued_at", nowNanos)
            .put("vector_clock", JSONObject().put("entries", JSONObject()))
            .put("correlation_id", JSONArray(UuidCodec.uuidToBytes(UuidCodec.randomUuid())))
            .put("causation_id", JSONObject.NULL)
            .put("payload", payload)
    }
}

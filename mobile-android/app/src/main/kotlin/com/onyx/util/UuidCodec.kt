package com.onyx.util

import java.security.SecureRandom
import java.util.Locale

/**
 * UUID string <-> 16-byte array conversion, matching
 * `platform-kernel::identifiers::ObjectId`'s real serde shape (a plain
 * derive on `struct ObjectId([u8; 16])`, confirmed by reading that file
 * directly) and Dart's identical `uuidToBytes`/`bytesToUuid`
 * (`mobile/lib/bridge/bridge.dart`) byte-for-byte: strip hyphens, parse each
 * hex-byte-pair left to right into the array, no reordering. Every real
 * FFI call into `mobile-core` that carries an id (`mobile_core_new`'s
 * `organization_id`, future command envelopes) needs this -- the HTTP
 * API returns ids as plain UUID strings (`LoginResponse.organization_id:
 * String`, confirmed in `api-server::routes::auth`), but the FFI config
 * JSON `ObjectId` expects a raw byte array, not a string.
 */
object UuidCodec {
    private val HEX_UUID = Regex("^[0-9a-fA-F]{32}$")

    fun uuidToBytes(uuid: String): IntArray {
        val normalized = uuid.replace("-", "")
        require(HEX_UUID.matches(normalized)) { "Invalid UUID: $uuid" }
        return IntArray(16) { index ->
            normalized.substring(index * 2, index * 2 + 2).toInt(16)
        }
    }

    fun bytesToUuid(bytes: IntArray): String {
        require(bytes.size == 16) { "Expected 16 bytes, got ${bytes.size}" }
        val hex = bytes.joinToString("") { String.format(Locale.ROOT, "%02x", it) }
        return "${hex.substring(0, 8)}-${hex.substring(8, 12)}-${hex.substring(12, 16)}-" +
            "${hex.substring(16, 20)}-${hex.substring(20)}"
    }

    /** Not currently needed by A3's own code paths, but kept alongside
     * the encode/decode pair above since `randomUuid()` is Dart's
     * equivalent third function in the same module and a future task
     * (command envelopes, A4+) will need it the same way Dart already
     * does for `command_id`/`operation_id`/`correlation_id`. */
    fun randomUuid(): String {
        val bytes = ByteArray(16)
        SecureRandom().nextBytes(bytes)
        bytes[6] = ((bytes[6].toInt() and 0x0f) or 0x40).toByte()
        bytes[8] = ((bytes[8].toInt() and 0x3f) or 0x80).toByte()
        val ints = IntArray(16) { bytes[it].toInt() and 0xff }
        return bytesToUuid(ints)
    }
}

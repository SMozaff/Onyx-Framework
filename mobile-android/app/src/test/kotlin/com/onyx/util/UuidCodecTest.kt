package com.onyx.util

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class UuidCodecTest {
    @Test
    fun `uuidToBytes matches Dart's real byte layout exactly`() {
        // Same fixture used in the real host-JVM proofs for A1/A3/A4:
        // 00010203-0405-0607-0809-0a0b0c0d0e0f -> [0,1,2,...,15].
        val bytes = UuidCodec.uuidToBytes("00010203-0405-0607-0809-0a0b0c0d0e0f")
        assertArrayEquals(IntArray(16) { it }, bytes)
    }

    @Test
    fun `bytesToUuid is the exact inverse of uuidToBytes`() {
        val original = "9f8e7d6c-5b4a-4392-8110-0f1e2d3c4b5a"
        val roundTripped = UuidCodec.bytesToUuid(UuidCodec.uuidToBytes(original))
        assertEquals(original, roundTripped)
    }

    @Test
    fun `uuidToBytes rejects a malformed UUID`() {
        assertThrows(IllegalArgumentException::class.java) { UuidCodec.uuidToBytes("not-a-uuid") }
    }

    @Test
    fun `randomUuid produces a real, valid, parseable v4 UUID`() {
        val uuid = UuidCodec.randomUuid()
        // Must not throw -- proves the generated string round-trips
        // through the same validator every real id does.
        UuidCodec.uuidToBytes(uuid)
        assertEquals('4', uuid[14])
    }
}

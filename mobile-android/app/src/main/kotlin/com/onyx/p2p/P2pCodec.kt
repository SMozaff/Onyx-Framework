package com.onyx.p2p

/**
 * Kotlin-side declarations for the Rust P2P transport codec
 * (`crates/mobile-android-jni/src/p2p.rs`), binding by JNI's standard
 * name-mangling convention (`Java_com_onyx_p2p_P2pCodec_native*`).
 *
 * The Rust side owns all intelligence (framing / encryption / handshake,
 * DECISIONS P2P-1); this `object` is deliberately pure marshalling — the
 * same no-business-logic discipline as `com.onyx.bridge.MobileCoreBridge`.
 *
 * Session lifecycle:
 * ```
 * val h = nativeSessionStart()                       // initiator
 * val clientMsg = nativeSessionClientMessage(h)!!    // -> over the wire
 * val h2 = nativeSessionAccept(clientMsg)            // responder (server)
 * val serverMsg = nativeSessionServerMessage(h2)!!   // -> over the wire
 * if (nativeSessionComplete(h, serverMsg) != 0) { /* handshake failed */ }
 * // both sides can now encode()/decode() frames
 * val frame = nativeSessionEncode(h, "Hello".toByteArray())!!
 * val plain = nativeSessionDecode(h2, frame)         // null => auth failed
 * nativeSessionClose(h); nativeSessionClose(h2)
 * ```
 *
 * Handles are opaque `Long`s minted by the Rust registry; `0`/`null` are
 * the documented failure sentinels (see the Rust module doc). A failed
 * `decode` permanently desynchronizes the stream — the caller must tear
 * down and re-handshake, never retry the same session.
 */
object P2pCodec {
    /** Initiator: create a pending session; returns a handle (`0` = failure). */
    external fun nativeSessionStart(): Long

    /** Initiator: the 65-byte uncompressed P-256 public point to send to the responder. */
    external fun nativeSessionClientMessage(handle: Long): ByteArray?

    /** Responder: consume the client's public point; returns a handle (`0` = failure). */
    external fun nativeSessionAccept(clientMessage: ByteArray): Long

    /** Responder: the 65-byte public point to send back to the initiator. */
    external fun nativeSessionServerMessage(handle: Long): ByteArray?

    /** Initiator: bind the server's public point; `0` ok, `-1` failure. */
    external fun nativeSessionComplete(handle: Long, serverMessage: ByteArray): Int

    /** Seal one record (`[len ‖ version ‖ ciphertext ‖ tag]`); null on failure. */
    external fun nativeSessionEncode(handle: Long, plaintext: ByteArray): ByteArray?

    /** Open one record; null on auth/malformed failure (stream is then broken). */
    external fun nativeSessionDecode(handle: Long, frame: ByteArray): ByteArray?

    /** Drop a session; `0` ok, `-1` unknown handle. */
    external fun nativeSessionClose(handle: Long): Int
}
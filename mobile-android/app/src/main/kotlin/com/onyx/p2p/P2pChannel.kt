package com.onyx.p2p

import java.io.ByteArrayOutputStream
import java.io.Closeable

/**
 * End-to-end delivery glue for one P2P connection: drives the
 * [P2pCodec] handshake, seals outbound messages into frames, and — the
 * part the drivers cannot know — reassembles inbound chunks back into
 * whole frames before decrypting, so the Rust codec's strict
 * length-checking `decode` (which must only ever receive a complete
 * frame) is never fed a partial record.
 *
 * Believed-synchronous usage per side:
 * ```
 * // Initiator:
 * val stream = wifiDirect.openStream(connectionInfo, role)  // or ble.*( )
 * val channel = P2pChannel(stream)
 * channel.setOnMessage { plaintext -> ... }
 * val clientMessage = channel.beginHandshake()
 * peerTransport.send(clientMessage)                        // any medium
 * val serverMessage = peerTransport.awaitServerMessage()   // any medium
 * channel.completeHandshake(serverMessage)
 * channel.send("Sync this".toByteArray())
 * ```
 * The opposite party calls [acceptHandshake] with the same two handshake
 * messages. Handshake messages themselves are deliberately plain bytes:
 * they travel over whatever transport the caller arranges (the drivers
 * below expose raw `write` for exactly this), because `P2pCodec` is
 * transport-agnostic.
 *
 * Failure model mirrors the Rust codec: a tampered/wrong-key frame breaks
 * the session permanently ([isBroken]). The caller must rebuild the whole
 * channel — never retry on a broken one.
 */
class P2pChannel(private val stream: P2pStream) : Closeable {
    @Volatile private var session: Long = 0L
    @Volatile private var ready: Boolean = false
    @Volatile private var closed: Boolean = false
    @Volatile private var broken: Boolean = false
    @Volatile private var onMessage: (ByteArray) -> Unit = {}

    private val buffer = ByteArrayOutputStream()

    init {
        stream.setOnBytes { chunk -> accumulate(chunk) }
    }

    /** Install the plaintext-message consumer (called on the stream's read thread). */
    fun setOnMessage(onMessage: (ByteArray) -> Unit) {
        this.onMessage = onMessage
    }

    val isBroken: Boolean get() = broken
    val isReady: Boolean get() = ready

    /**
     * Initiator role: create the Rust session and return the 65-byte public
     * point to deliver to the peer.
     */
    fun beginHandshake(): ByteArray {
        check(session == 0L) { "handshake already begun" }
        val handle = P2pCodec.nativeSessionStart()
        check(handle != 0L) { "P2P codec failed to create initiator session" }
        session = handle
        return P2pCodec.nativeSessionClientMessage(handle)
            ?: throw IllegalStateException("P2P codec produced no handshake message")
    }

    /**
     * Responder role: consume the initiator's public point, create the ready
     * responder session, and return the 65-byte reply the initiator needs.
     */
    fun acceptHandshake(clientMessage: ByteArray): ByteArray {
        check(session == 0L) { "handshake already begun" }
        val handle = P2pCodec.nativeSessionAccept(clientMessage)
        check(handle != 0L) { "P2P codec rejected the initiator's handshake message" }
        session = handle
        ready = true // responder's keys are derived during accept
        return P2pCodec.nativeSessionServerMessage(handle)
            ?: throw IllegalStateException("P2P codec produced no handshake reply")
    }

    /** Initiator role: bind the responder's public point, completing the handshake. */
    fun completeHandshake(serverMessage: ByteArray) {
        check(session != 0L) { "beginHandshake must precede completeHandshake" }
        check(P2pCodec.nativeSessionComplete(session, serverMessage) == 0) {
            "P2P handshake failed — peer rejected our session"
        }
        ready = true
    }

    /**
     * Seal `message` and ship it as one frame. Returns `false` if the codec
     * refused to encode (not ready, or session already broken).
     */
    fun send(message: ByteArray): Boolean {
        if (!ready) return false
        val frame = P2pCodec.nativeSessionEncode(session, message) ?: return false
        stream.write(frame)
        return true
    }

    override fun close() {
        if (closed) return
        closed = true
        if (session != 0L) P2pCodec.nativeSessionClose(session)
        runCatching { stream.close() }
    }

    private fun accumulate(chunk: ByteArray) {
        if (broken || closed) return
        buffer.write(chunk)
        try {
            extractFrames()
        } catch (_: IllegalStateException) {
            breakStream()
        }
    }

    /** Pull as many complete frames out of the buffer as are available. */
    private fun extractFrames() {
        while (!broken && !closed) {
            val data = buffer.toByteArray()
            if (data.size < HEADER_LEN) return
            val declared = readLength(data)
            if (declared > MAX_FRAME) throw IllegalStateException("P2P frame too large ($declared bytes)")
            val frameLen = HEADER_LEN + declared + TAG_LEN
            if (data.size < frameLen) return
            val frame = data.copyOfRange(0, frameLen)
            val remainder = data.copyOfRange(frameLen, data.size)
            buffer.reset()
            buffer.write(remainder)
            val plaintext = P2pCodec.nativeSessionDecode(session, frame)
                ?: throw IllegalStateException("P2P frame failed authentication — session desynchronized")
            onMessage(plaintext)
        }
    }

    private fun readLength(data: ByteArray): Int =
        ((data[0].toInt() and 0xff) shl 24) or
            ((data[1].toInt() and 0xff) shl 16) or
            ((data[2].toInt() and 0xff) shl 8) or
            (data[3].toInt() and 0xff)

    private fun breakStream() {
        broken = true
        runCatching { stream.close() }
    }

    private companion object {
        const val HEADER_LEN = 5 // 4-byte length prefix + 1-byte version
        const val TAG_LEN = 16 // AES-256-GCM tag
        const val MAX_FRAME = 1 shl 20 // 1 MiB sanity cap
    }
}
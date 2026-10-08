package com.onyx.p2p

import java.io.Closeable
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket

/**
 * A reliable, in-order byte stream — the transport contract the Rust codec
 * (see [P2pCodec]) requires: `write`s arrive at the peer in order and the
 * [onBytes] delivery is sequential. The two Phase 4.1 platform drivers
 * provide this over their respective media:
 *   * [WifiDirectDriver] — a plain TCP `Socket`/`ServerSocket.accept()`
 *     pair on the Wi-Fi Direct group owner address.
 *   * [BleDriver] — a GATT notification stream (server→client) plus a
 *     write-characteristic stream (client→server).
 *
 * Data crosses the boundary as arbitrary chunks; framing/decryption is
 * [P2pChannel]'s responsibility, not the media's.
 */
interface P2pStream : Closeable {
    /** Enqueue one chunk for the peer. Sequential calls arrive in order. */
    fun write(bytes: ByteArray)

    /** Install the chunk consumer. Only one consumer is supported. */
    fun setOnBytes(onBytes: (ByteArray) -> Unit)

    override fun close()
}

/**
 * Streaming implementation over a connected `Socket` (owner role) or a
 * `ServerSocket.accept()` connection (client role, Wi-Fi Direct). The read
 * loop is sequential by construction (a single thread), so delivery order
 * is exactly the wire order.
 */
class SocketStream(
    private val socket: Socket,
    private val initialOnBytes: (ByteArray) -> Unit = {},
) : P2pStream {
    private val input: InputStream = socket.getInputStream()
    private val output: OutputStream = socket.getOutputStream()
    @Volatile private var closed = false
    @Volatile private var onBytes: (ByteArray) -> Unit = initialOnBytes
    private var reader: Thread? = null

    /** Starts the blocking read loop. Must be called before bytes arrive. */
    fun start() {
        reader = Thread {
            try {
                val buf = ByteArray(READ_CHUNK)
                while (!closed) {
                    val n = input.read(buf)
                    if (n < 0) {
                        close()
                        break
                    }
                    if (n > 0) onBytes(buf.copyOf(n))
                }
            } catch (_: Exception) {
                close()
            }
        }.apply {
            isDaemon = true
            name = "onyx-p2p-socket-read"
            start()
        }
    }

    override fun setOnBytes(onBytes: (ByteArray) -> Unit) {
        this.onBytes = onBytes
    }

    override fun write(bytes: ByteArray) {
        if (closed) throw IllegalStateException("P2P socket is closed")
        output.write(bytes)
        output.flush()
    }

    override fun close() {
        if (closed) return
        closed = true
        try {
            reader?.join(500)
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
        }
        runCatching { socket.close() }
    }

    private companion object {
        const val READ_CHUNK = 4096
    }
}
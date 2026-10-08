package com.onyx.p2p

import android.content.Context
import android.net.wifi.p2p.WifiP2pInfo
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

private const val TAG = "P2pController"

/** The two Phase 4.1 media the drivers speak. */
enum class P2pTransport(val label: String) {
    BLE("Bluetooth LE"),
    WIFI_DIRECT("Wi-Fi Direct"),
}

/** A discoverable/connected remote for the Settings P2P card. */
data class P2pPeer(val id: String, val name: String, val transport: P2pTransport)

/** The controller's current session state, surfaced for the Settings card. */
sealed interface P2pStatus {
    data object Idle : P2pStatus
    data object Starting : P2pStatus
    data object Advertising : P2pStatus
    data object Discovering : P2pStatus
    data object Connecting : P2pStatus
    data class Connected(val peer: String) : P2pStatus
    data class Error(val message: String) : P2pStatus
}

/** One entry in the probe-message transcript (inbound or outbound). */
data class P2pMessage(
    val text: String,
    val incoming: Boolean,
    val timestamp: Long = System.currentTimeMillis(),
)

/**
 * The missing headless wiring for `com.onyx.p2p` (MIGRATION_PLAN Phase
 * 4.1, DECISIONS P2P-1): owns the [BleDriver] and [WifiDirectDriver]
 * platform drivers, drives a [P2pChannel] over whichever medium a peer
 * arrives on, exchanges the Rust codec's two handshake public points over
 * the raw stream (the [P2pChannel.deferredHandshake] construction), and
 * turns plaintext messages into a [P2pMessage] log. No business logic of
 * its own — it is the same media-coordination layer Dart gets from its
 * platform channels, with the intelligence staying in the Rust codec.
 *
 * # Topology
 * One device runs [startServer] (BLE advertiser / Wi-Fi Direct group
 * owner acceptor), the other [connectBle] or [connectWifi]. The BLE path
 * is fully driven here (auto-connect on the fixed service UUID); the
 * Wi-Fi Direct path drives discovery, connection and group formation via
 * the driver's own broadcasts and `requestConnectionInfo` polling, then
 * opens the group-owner TCP stream. Both media end at the same
 * [runHandshake]/[send] path.
 *
 * # Device truth is deferred (Phase 4.1b/d)
 * Every branch here is hand-checked against the driver code; the actual
 * two-device session is the on-device lab exercise still tracked in the
 * MIGRATION_PLAN. In particular the Wi-Fi Direct connection callback
 * cadence and the exact BLE MTU behavior are device-dependent and are
 * not asserted by any automated test on this box.
 */
class P2pController(context: Context) {
    private val appContext = context.applicationContext
    private val ble = BleDriver(appContext)
    private val wifi = WifiDirectDriver(appContext)
    private val executor: ExecutorService =
        Executors.newSingleThreadExecutor { r -> Thread(r, "onyx-p2p").apply { isDaemon = true } }

    private val _status = MutableStateFlow<P2pStatus>(P2pStatus.Idle)
    val status: StateFlow<P2pStatus> = _status.asStateFlow()

    private val _peers = MutableStateFlow<List<P2pPeer>>(emptyList())
    val peers: StateFlow<List<P2pPeer>> = _peers.asStateFlow()

    private val _messages = MutableStateFlow<List<P2pMessage>>(emptyList())
    val messages: StateFlow<List<P2pMessage>> = _messages.asStateFlow()

    @Volatile private var channel: P2pChannel? = null
    private val active = AtomicBoolean(false)

    /** The union of runtime permissions the selected medium needs (UI gates on these). */
    fun permissionsFor(transport: P2pTransport): List<String> =
        when (transport) {
            P2pTransport.BLE -> ble.serverPermissions().plus(ble.clientPermissions()).distinct()
            P2pTransport.WIFI_DIRECT -> wifi.runtimePermissions().toList()
        }

    // ---------------------------------------------------------------- server

    /** Become the acceptor: BLE advertiser (responder role in the codec handshake). */
    fun startServer() {
        if (!canStart(P2pTransport.BLE)) {
            _status.value = P2pStatus.Error("Bluetooth is off or the runtime permission was not granted")
            return
        }
        _status.value = P2pStatus.Starting
        executor.execute {
            ble.startServer { stream ->
                if (stream == null) {
                    fail("BLE advertise failed (radio unavailable or permission revoked)")
                } else {
                    executor.execute { runHandshake(stream, initiator = false, peer = "BLE peer") }
                }
            }
        }
    }

    // ---------------------------------------------------------------- connect

    /** Client role over BLE: scan for the fixed service UUID and connect. */
    fun connectBle() {
        if (!canStart(P2pTransport.BLE)) {
            _status.value = P2pStatus.Error("Bluetooth is off, scanning is gated, or the permission was not granted")
            return
        }
        if (!active.compareAndSet(false, true)) return
        _status.value = P2pStatus.Connecting
        executor.execute {
            ble.scanAndConnect { stream ->
                if (stream == null) {
                    fail("BLE scan found no Onyx advertiser")
                } else {
                    executor.execute { runHandshake(stream, initiator = true, peer = "BLE peer") }
                }
            }
        }
    }

    /** Discover the Wi-Fi Direct peer list; connect via [connectWifi]. */
    fun discoverWifiPeers() {
        if (!canStart(P2pTransport.WIFI_DIRECT)) {
            _status.value = P2pStatus.Error("Wi-Fi Direct is unavailable or the runtime permission was not granted")
            return
        }
        _status.value = P2pStatus.Discovering
        executor.execute {
            wifi.discoverPeers { ok ->
                executor.execute {
                    if (!ok) {
                        fail("Wi-Fi Direct discovery could not start (radio or permission)")
                    } else {
                        wifi.requestPeers { devices ->
                            executor.execute {
                                _peers.value = devices.map {
                                    P2pPeer(it.deviceAddress, it.deviceName.ifBlank { it.deviceAddress }, P2pTransport.WIFI_DIRECT)
                                }
                                _status.value = if (_peers.value.isEmpty()) P2pStatus.Idle else P2pStatus.Discovering
                            }
                        }
                    }
                }
            }
        }
    }

    /** Connect to a previously-discovered Wi-Fi Direct peer (client role, initiator). */
    fun connectWifi(peer: P2pPeer) {
        if (!canStart(P2pTransport.WIFI_DIRECT)) {
            _status.value = P2pStatus.Error("Wi-Fi Direct is unavailable or the runtime permission was not granted")
            return
        }
        if (!active.compareAndSet(false, true)) return
        _status.value = P2pStatus.Connecting
        executor.execute {
            wifi.requestPeers { devices ->
                val device = devices.firstOrNull { it.deviceAddress == peer.id }
                if (device == null) {
                    fail("Wi-Fi Direct peer '${peer.name}' is no longer visible")
                    return@requestPeers
                }
                wifi.connectTo(device) { ok ->
                    executor.execute {
                        if (!ok) {
                            fail("Wi-Fi Direct connect request failed")
                        } else {
                            val info = awaitGroupFormed(GROUP_FORM_TIMEOUT_MS)
                            if (info == null) {
                                fail("Wi-Fi Direct group never formed (two devices are required — Phase 4.1 on-device work)")
                            } else {
                                val stream = runCatching { wifi.openStream(info) }.getOrNull()
                                if (stream == null) {
                                    fail("Wi-Fi Direct stream failed to open (no group owner address or accept error)")
                                } else {
                                    runHandshake(stream, initiator = !info.isGroupOwner, peer = peer.name)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- session

    /** Seal [text] into a frame and ship it (probe message). No-op when not connected. */
    fun send(text: String): Boolean {
        val ch = channel ?: return false
        if (!ch.isReady) return false
        val ok = ch.send(text.encodeToByteArray())
        if (ok) _messages.value += P2pMessage(text, incoming = false)
        return ok
    }

    /** Tear down this session's link; the controller stays usable. */
    fun stop() {
        active.set(false)
        executor.execute {
            runCatching { channel?.close() }
            channel = null
            _status.value = P2pStatus.Idle
        }
    }

    /** Full teardown for the owning ViewModel's [onCleared]. */
    fun release() {
        stop()
        runCatching { wifi.destroy() }
        executor.shutdown()
    }

    // ---------------------------------------------------------------- internals

    private fun canStart(transport: P2pTransport): Boolean =
        when (transport) {
            P2pTransport.BLE -> ble.canStart()
            P2pTransport.WIFI_DIRECT -> wifi.canStart()
        }

    private fun runHandshake(stream: P2pStream, initiator: Boolean, peer: String) {
        val ch = P2pChannel(stream, deferredHandshake = true)
        val publicPoint = LatchHolder()
        ch.setRawReader(HANDSHAKE_POINT_SIZE) { bytes ->
            if (publicPoint.bytes == null) {
                publicPoint.bytes = bytes
                publicPoint.latch.countDown()
            }
        }
        ch.setOnMessage { plaintext -> onIncoming(plaintext) }
        channel = ch
        try {
            if (initiator) {
                val clientMessage = ch.beginHandshake()
                stream.write(clientMessage)
                if (!awaitHandshake(publicPoint)) return fail("Timed out waiting for the responder's public point")
                ch.completeHandshake(publicPoint.bytes!!)
            } else {
                if (!awaitHandshake(publicPoint)) return fail("Timed out waiting for the initiator's public point")
                val serverMessage = ch.acceptHandshake(publicPoint.bytes!!)
                stream.write(serverMessage)
            }
            ch.startFraming()
            _status.value = P2pStatus.Connected(peer)
        } catch (e: Exception) {
            Log.w(TAG, "handshake failed over $peer", e)
            fail("Handshake failed (${e.message ?: e::class.java.simpleName})")
        }
    }

    private fun awaitHandshake(holder: LatchHolder): Boolean =
        runCatching { holder.latch.await(HANDSHAKE_TIMEOUT_MS, TimeUnit.MILLISECONDS) }.getOrDefault(false)

    private fun awaitGroupFormed(timeoutMs: Long): WifiP2pInfo? {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            var info: WifiP2pInfo? = null
            val latch = CountDownLatch(1)
            wifi.requestConnectionInfo { i -> info = i; latch.countDown() }
            latch.await(1, TimeUnit.SECONDS)
            if (info?.groupFormed == true) return info
        }
        return null
    }

    private fun onIncoming(plaintext: ByteArray) {
        _messages.value += P2pMessage(plaintext.decodeToString(), incoming = true)
    }

    private fun fail(message: String) {
        active.set(false)
        Log.w(TAG, message)
        runCatching { channel?.close() }
        channel = null
        _status.value = P2pStatus.Error(message)
    }

    private class LatchHolder {
        val latch = CountDownLatch(1)
        @Volatile var bytes: ByteArray? = null
    }

    private companion object {
        /** Uncompressed P-256 public point (65 bytes), per the Rust codec contract. */
        const val HANDSHAKE_POINT_SIZE = 65
        const val HANDSHAKE_TIMEOUT_MS = 10_000L
        const val GROUP_FORM_TIMEOUT_MS = 15_000L
    }
}
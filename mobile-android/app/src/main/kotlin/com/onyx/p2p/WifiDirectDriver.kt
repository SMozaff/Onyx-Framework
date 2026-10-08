package com.onyx.p2p

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.net.wifi.p2p.WifiP2pConfig
import android.net.wifi.p2p.WifiP2pDevice
import android.net.wifi.p2p.WifiP2pInfo
import android.net.wifi.p2p.WifiP2pManager
import android.os.Build
import android.os.Looper
import androidx.core.content.ContextCompat
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket

/**
 * Wi-Fi Direct platform driver (MIGRATION_PLAN Phase 4.1, DECISIONS P2P-1):
 * Kotlin owns the `WifiP2pManager` platform integration — discovery,
 * connection, and the group-owner TCP socket that becomes the [P2pStream]
 * the Rust codec runs over. All intelligence (framing/encryption/
 * handshake) stays in `P2pCodec`/`P2pChannel`; this class is deliberately
 * a media driver and nothing more.
 *
 * Transport shape: the group owner runs a `ServerSocket` on fixed [PORT];
 * the client connects a `Socket` to `WifiP2pInfo.groupOwnerAddress`. TCP
 * gives the reliable, in-order stream the codec's strict per-direction
 * counters require.
 *
 * # Permission model
 * * API 33+: runtime `NEARBY_WIFI_DEVICES` (plus the normal,
 *   auto-granted `ACCESS_WIFI_STATE` / `CHANGE_WIFI_STATE`).
 * * API 29-32: runtime `ACCESS_FINE_LOCATION`, same normal-state perms.
 * [canStart] is the gate; the UI layer must request the runtime permission
 * before calling [discoverPeers]/[connectTo].
 *
 * # Device verification
 * Instrumented/on-device exercise is Phase 4.1(d) and is deferred — this
 * class compiles only in CI (no Android SDK on the dev box); its runtime
 * truth is a two-device lab run, tracked in MIGRATION_PLAN Phase 4.1.
 */
class WifiDirectDriver(context: Context) {
    private val appContext = context.applicationContext
    private val manager: WifiP2pManager? =
        appContext.getSystemService(Context.WIFI_P2P_SERVICE) as? WifiP2pManager
    private var channel: WifiP2pManager.Channel? = null
    private var receiver: BroadcastReceiver? = null

    /** Fired on any Wi-Fi Direct state/peer/connection broadcast. */
    @Volatile var onStateChanged: (() -> Unit)? = null

    init {
        channel = manager?.initialize(
            appContext,
            Looper.getMainLooper(),
        ) { /* channel lost — surfaced via onStateChanged / discover results */ }
        registerReceiver()
    }

    /** True when the runtime permissions the current API level needs are held. */
    fun canStart(): Boolean = runtimePermissions().all {
        ContextCompat.checkSelfPermission(appContext, it) == PackageManager.PERMISSION_GRANTED
    }

    /** The runtime (dangerous) permissions the caller must already hold. */
    fun runtimePermissions(): Array<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            arrayOf(Manifest.permission.NEARBY_WIFI_DEVICES)
        } else {
            arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
        }

    /** Start an async discovery window; results arrive via [requestPeers]. */
    fun discoverPeers(onResult: (Boolean) -> Unit) {
        val actionListener = object : WifiP2pManager.ActionListener {
            override fun onSuccess() = onResult(true)
            override fun onFailure(reason: Int) = onResult(false)
        }
        manager?.discoverPeers(channel, actionListener) ?: onResult(false)
    }

    /** Pull the current discovered-device list (call after a discovery window). */
    fun requestPeers(onPeers: (List<WifiP2pDevice>) -> Unit) {
        manager?.requestPeers(channel) { peers ->
            onPeers(peers.deviceList.toList())
        }
    }

    /** Open a p2p group with [device] and drive the connection state machine. */
    fun connectTo(device: WifiP2pDevice, onResult: (Boolean) -> Unit) {
        val config = WifiP2pConfig().apply { deviceAddress = device.deviceAddress }
        val actionListener = object : WifiP2pManager.ActionListener {
            override fun onSuccess() = onResult(true)
            override fun onFailure(reason: Int) = onResult(false)
        }
        manager?.connect(channel, config, actionListener) ?: onResult(false)
    }

    /**
     * Resolve the formed group's `WifiP2pInfo` (owner address + role).
     * Callers poll via [connectionChanged] or a short retry until
     * `info.groupFormed` is true.
     */
    fun requestConnectionInfo(onInfo: (WifiP2pInfo) -> Unit) {
        manager?.requestConnectionInfo(channel, onInfo)
    }

    /**
     * Once `info.groupFormed` is true, produce the [P2pStream] for this
     * device's role: the group owner accepts a client on [PORT], the client
     * connects out to `groupOwnerAddress`. The returned stream is already
     * reading in the background.
     */
    fun openStream(info: WifiP2pInfo): P2pStream {
        val stream: SocketStream = if (info.isGroupOwner) {
            val server = ServerSocket(PORT)
            SocketStream(server.accept()).also { server.close() }
        } else {
            requireNotNull(info.groupOwnerAddress) { "groupOwnerAddress must be set for a client" }
            SocketStream(Socket(info.groupOwnerAddress, PORT))
        }
        stream.start()
        return stream
    }

    /** Convenience for an explicit owner address (used by instrumented tests). */
    fun openStreamTo(owner: InetAddress): P2pStream {
        val stream = SocketStream(Socket(owner, PORT))
        stream.start()
        return stream
    }

    /** Tear the manager channel and unregister the broadcast receiver. */
    fun destroy() {
        runCatching { appContext.unregisterReceiver(receiver) }
        receiver = null
        manager?.removeGroup(channel, null)
        channel = null
    }

    private fun registerReceiver() {
        receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                onStateChanged?.invoke()
            }
        }
        val filter = IntentFilter().apply {
            addAction(WifiP2pManager.WIFI_P2P_STATE_CHANGED_ACTION)
            addAction(WifiP2pManager.WIFI_P2P_PEERS_CHANGED_ACTION)
            addAction(WifiP2pManager.WIFI_P2P_CONNECTION_CHANGED_ACTION)
            addAction(WifiP2pManager.WIFI_P2P_THIS_DEVICE_CHANGED_ACTION)
        }
        appContext.registerReceiver(receiver, filter)
    }

    private companion object {
        /** Fixed service port for the p2p group's TCP stream. */
        const val PORT = 47015
    }
}
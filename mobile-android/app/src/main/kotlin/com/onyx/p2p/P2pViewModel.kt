package com.onyx.p2p

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import kotlinx.coroutines.flow.StateFlow

/**
 * Activity-scoped host for [P2pController] so the Settings P2P card
 * survives recomposition with the controller outliving any one frame.
 * Constructed by Compose's default factory via its `(Application)`
 * constructor; its only job is to forward the controller's StateFlows and
 * lifecycle ([onCleared] → [P2pController.release]).
 */
class P2pViewModel(application: Application) : AndroidViewModel(application) {
    private val controller = P2pController(application)

    val status: StateFlow<P2pStatus> = controller.status
    val peers: StateFlow<List<P2pPeer>> = controller.peers
    val messages: StateFlow<List<P2pMessage>> = controller.messages

    fun permissionsFor(transport: P2pTransport): List<String> = controller.permissionsFor(transport)
    fun startServer() = controller.startServer()
    fun connectBle() = controller.connectBle()
    fun discoverWifiPeers() = controller.discoverWifiPeers()
    fun connectWifi(peer: P2pPeer) = controller.connectWifi(peer)
    fun send(text: String): Boolean = controller.send(text)
    fun stop() = controller.stop()

    override fun onCleared() {
        controller.release()
    }
}
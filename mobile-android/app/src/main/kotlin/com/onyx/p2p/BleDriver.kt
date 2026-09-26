package com.onyx.p2p

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import androidx.core.content.ContextCompat
import java.util.UUID

/**
 * BLE platform driver (MIGRATION_PLAN Phase 4.1, DECISIONS P2P-1): Kotlin
 * owns `BluetoothLeScanner`/`BluetoothLeAdvertiser` + GATT; the Rust codec
 * runs its framing/encryption over the resulting ordered byte stream.
 *
 * Topology: one device is the *server* (advertises a service UUID, runs a
 * GATT server, pushes bytes via notifications), the other is the *client*
 * (scans for the UUID, connects, pushes bytes via a write characteristic).
 * Writes/notifications are delivered in order on each link, which is the
 * transport contract [P2pChannel] needs; payload is chunked at [MTU_PAYLOAD]
 * (unnegotiated 20-byte MTU — MTU upgrade is part of the deferred
 * on-device work, MIGRATION_PLAN Phase 4.1b) and reassembled by the
 * channel's frame buffer.
 *
 * # Permission model
 * * API 31+: `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`, `BLUETOOTH_ADVERTISE`.
 * * API 29-30 (legacy): `BLUETOOTH`, `BLUETOOTH_ADMIN` (app manifest) and
 *   runtime `ACCESS_FINE_LOCATION` (scanning/beacon detection).
 * The runtime-gated permission you actually need depends on role:
 * [serverPermissions] / [clientPermissions].
 *
 * # Device verification
 * Instrumented/on-device exercise is Phase 4.1(d) and is deferred — this
 * class compiles only in CI (no Android SDK on the dev box); its runtime
 * truth is a two-device lab run, tracked in MIGRATION_PLAN Phase 4.1.
 */
@SuppressLint("MissingPermission") // gates are explicit via canStart()/canScan()
class BleDriver(private val context: Context) {
    private val appContext = context.applicationContext
    private val bluetoothManager: BluetoothManager? =
        appContext.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
    private val adapter: BluetoothAdapter? = bluetoothManager?.adapter

    private val serverUri = BleUris.service
    private val server = BluetoothGattService(serverUri, BluetoothGattService.SERVICE_TYPE_PRIMARY)
    private val rxCharacteristic =
        BluetoothGattCharacteristic(
            BleUris.rxWrite,
            BluetoothGattCharacteristic.PROPERTY_WRITE,
            BluetoothGattCharacteristic.PERMISSION_WRITE,
        )
    private val txCharacteristic =
        BluetoothGattCharacteristic(
            BleUris.txNotify,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_READ,
        ).apply {
            addDescriptor(
                BluetoothGattDescriptor(
                    BleUris.clientCharacteristicConfig,
                    BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
                ),
            )
        }

    private var gattServer: BluetoothGattServer? = null
    private var gattClient: BluetoothGatt? = null

    /** Cross-role incoming-bytes sink; wired by the returned [BleStream]. */
    @Volatile private var onIncoming: (ByteArray) -> Unit = {}
    @Volatile private var peerDevice: BluetoothDevice? = null

    /** True when the adapter can be used at all (enabled + permission gate). */
    fun canStart(): Boolean =
        adapter?.isEnabled == true &&
            contextHas(if (Build.VERSION.SDK_INT >= 31) Manifest.permission.BLUETOOTH_ADVERTISE else Manifest.permission.BLUETOOTH)

    /** True when scanning is possible (client role). */
    fun canScan(): Boolean =
        if (Build.VERSION.SDK_INT >= 31) {
            contextHas(Manifest.permission.BLUETOOTH_SCAN)
        } else {
            contextHas(Manifest.permission.ACCESS_FINE_LOCATION)
        }

    /** The runtime permissions a server-role caller must hold. */
    fun serverPermissions(): Array<String> =
        if (Build.VERSION.SDK_INT >= 31) arrayOf(Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT)
        else emptyArray()

    /** The runtime permissions a client-role caller must hold. */
    fun clientPermissions(): Array<String> =
        if (Build.VERSION.SDK_INT >= 31) arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT)
        else arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)

    /**
     * Server role. Advertises [BleUris.service] and opens a GATT server so a
     * scanner can connect. Returns the [P2pStream] over notifications
     * (server→client) and the write characteristic (client→server).
     */
    fun startServer(onResult: (P2pStream?) -> Unit) {
        if (!canStart()) return onResult(null)
        val advertiser = adapter?.bluetoothLeAdvertiser
        val gatt = bluetoothManager?.openGattServer(appContext, serverCallback)
        if (advertiser == null || gatt == null) return onResult(null)

        gattServer = gatt
        server.apply {
            addCharacteristic(rxCharacteristic)
            addCharacteristic(txCharacteristic)
        }
        gatt.addService(server)
        advertiser.advertise(
            AdvertiseSettings.Builder()
                .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
                .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_HIGH)
                .setConnectable(true)
                .build(),
            AdvertiseData.Builder().addServiceUuid(ParcelUuid(serverUri)).build(),
            object : AdvertiseCallback() {
                override fun onStartSuccess(settingsInEffect: AdvertiseSettings?) = Unit
                override fun onStartFailure(errorCode: Int) {
                    onResult(null)
                }
            },
        )
        onResult(BleStream(this, isServer = true))
    }

    /**
     * Client role. Scans (with a service-uuid filter) and connects to the
     * first advertiser of [BleUris.service]; then returns the [P2pStream]
     * once services are discovered and notifications are enabled.
     */
    fun scanAndConnect(onResult: (P2pStream?) -> Unit) {
        if (!canStart() || !canScan()) return onResult(null)
        val scanner = adapter?.bluetoothLeScanner ?: return onResult(null)
        val filter = ScanFilter.Builder().setServiceUuid(ParcelUuid(serverUri)).build()
        scanner.startScan(
            listOf(filter),
            ScanSettings.Builder()
                .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                .build(),
            object : ScanCallback() {
                override fun onScanResult(callbackType: Int, result: ScanResult) {
                    scanner.stopScan(this)
                    connect(result.device, onResult)
                }

                override fun onScanFailed(errorCode: Int) {
                    onResult(null)
                }
            },
        )
    }

    private fun connect(device: BluetoothDevice, onResult: (P2pStream?) -> Unit) {
        peerDevice = device
        gattClient = device.connectGatt(
            appContext,
            false,
            object : BluetoothGattCallback() {
                override fun onConnectionStateChange(
                    gatt: BluetoothGatt,
                    status: Int,
                    newState: Int,
                ) {
                    if (newState != BluetoothProfile.STATE_CONNECTED) return
                    if (status != BluetoothGatt.GATT_SUCCESS) return onResult(null)
                    gatt.discoverServices()
                }

                override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                    if (status != BluetoothGatt.GATT_SUCCESS) return onResult(null)
                    val service = gatt.getService(BleUris.service) ?: return onResult(null)
                    val tx = service.getCharacteristic(BleUris.txNotify) ?: return onResult(null)
                    val descriptor = tx.getDescriptor(BleUris.clientCharacteristicConfig) ?: return onResult(null)
                    gatt.setCharacteristicNotification(tx, true)
                    descriptor.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                    gatt.writeDescriptor(descriptor)
                    onResult(BleStream(this@BleDriver, isServer = false))
                }

                override fun onCharacteristicChanged(
                    gatt: BluetoothGatt,
                    characteristic: BluetoothGattCharacteristic,
                    value: ByteArray,
                ) {
                    onIncoming(value)
                }
            },
        )
    }

    /**
     * A [P2pStream] over one BLE link. Outbound [write] chunks at
     * [MTU_PAYLOAD]; inbound arrives via GATT callbacks (notifications on
     * the client, write requests on the server), delivered in order to
     * [setOnBytes].
     */
    inner class BleStream(
        private val driver: BleDriver,
        private val isServer: Boolean,
    ) : P2pStream {
        @Volatile private var closed = false
        @Volatile private var onBytes: (ByteArray) -> Unit = {}

        override fun write(bytes: ByteArray) {
            if (closed) throw IllegalStateException("BLE stream is closed")
            bytes.asList().chunked(MTU_PAYLOAD).forEach { chunk ->
                val payload = chunk.toByteArray()
                if (isServer) {
                    driver.peerDevice?.let { device ->
                        driver.gattServer?.notifyCharacteristicChanged(
                            device,
                            driver.txCharacteristic,
                            false,
                            payload,
                        )
                    }
                } else {
                    driver.gattClient?.let { gatt ->
                        val rx = gatt
                            .getService(BleUris.service)
                            ?.getCharacteristic(BleUris.rxWrite)
                        rx?.let {
                            it.value = payload
                            gatt.writeCharacteristic(it)
                        }
                    }
                }
            }
        }

        override fun setOnBytes(onBytes: (ByteArray) -> Unit) {
            this.onBytes = onBytes
            driver.onIncoming = onBytes
        }

        override fun close() {
            if (closed) return
            closed = true
            driver.teardown()
        }
    }

    private fun teardown() {
        runCatching { gattClient?.disconnect() }
        runCatching {
            peerDevice?.let { gattServer?.cancelConnection(it) }
        }
        runCatching { gattServer?.clearServices() }
        runCatching { gattServer?.close() }
        gattServer = null
        gattClient = null
        peerDevice = null
    }

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) peerDevice = device
            if (newState == BluetoothProfile.STATE_DISCONNECTED && peerDevice == device) peerDevice = null
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (responseNeeded) {
                gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, value)
            }
            onIncoming(value)
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            // Nothing to serve on read; the link is write/notify only.
            gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
        }
    }

    private fun contextHas(permission: String): Boolean =
        ContextCompat.checkSelfPermission(appContext, permission) == PackageManager.PERMISSION_GRANTED

    private companion object {
        /** Unnegotiated ATT MTU payload (23 - 3 header bytes); MTU upgrade deferred. */
        const val MTU_PAYLOAD = 20
    }
}

private object BleUris {
    /** Onyx P2P service — fixed constant so both devices discover each other. */
    @Suppress("unused")
    val service: UUID = UUID.fromString("8d3a9b2e-6f5c-4a1e-9b8d-2e4f6a8c0b1d")
    /** Client→server data (write characteristic). */
    val rxWrite: UUID = UUID.fromString("8d3a9b2e-6f5c-4a1e-9b8d-2e4f6a8c0b1e")
    /** Server→client data (notify characteristic). */
    val txNotify: UUID = UUID.fromString("8d3a9b2e-6f5c-4a1e-9b8d-2e4f6a8c0b1f")
    /** Standard CCCD descriptor for enabling notifications. */
    val clientCharacteristicConfig: UUID =
        UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
}
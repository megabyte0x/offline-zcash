@file:Suppress("DEPRECATION", "MissingPermission")
package org.zcash.offline

import android.bluetooth.*
import android.bluetooth.le.*
import android.content.Context
import android.os.*
import org.json.JSONObject
import uniffi.ozm.LinkSink
import uniffi.ozm.OfflineNode
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong

/** Original Android implementation; no BitChat code. Only byte transport lives here. */
class BleLink(context: Context, private val node: OfflineNode, private val report: (String) -> Unit) : LinkSink {
    companion object {
        val SERVICE: UUID = UUID.fromString("b91d6e31-89f5-4a38-9761-72d1305a4f12")
        val STREAM: UUID = UUID.fromString("b91d6e32-89f5-4a38-9761-72d1305a4f12")
        val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    }
    private val context = context.applicationContext
    private val manager = context.getSystemService(BluetoothManager::class.java)
    private val adapter get() = manager.adapter
    private val thread = HandlerThread("ozm-ble").apply { start() }
    private val handler = Handler(thread.looper)
    private val ids = AtomicLong()
    private val links = ConcurrentHashMap<ULong, Conn>()
    private var server: BluetoothGattServer? = null
    private var characteristic: BluetoothGattCharacteristic? = null
    private var scanning = false
    private var advertising = false
    private var foreground = true
    private var stopped = false
    // Zero service-data hint: every Offline Zcash device connects and relays (older peers accept zero).
    private val hint = ByteArray(4)

    private inner class Conn(val device: BluetoothDevice, val central: Boolean) {
        val id = ids.incrementAndGet().toULong()
        var gatt: BluetoothGatt? = null
        var remote: BluetoothGattCharacteristic? = null
        @Volatile var mtu = 23
        var lastTraffic = SystemClock.elapsedRealtime()
        val ready = CountDownLatch(1)
        @Volatile var opened = false
        @Volatile var closed = false
        var discovering = false
        var subscribed = false
        var complete: ((Boolean) -> Unit)? = null
        // Android GATT_MAX_ATTR_LEN is 512 even when ATT MTU - 3 is 514.
        val writer = ChunkWriter({ minOf(mtu - 3, 512) }) { bytes, done ->
            handler.post {
                if (closed) { done(false); return@post }
                complete = done
                val accepted = if (central) {
                    val c = remote!!
                    if (Build.VERSION.SDK_INT >= 33) gatt?.writeCharacteristic(c, bytes, BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE) == BluetoothStatusCodes.SUCCESS
                    else {
                        c.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
                        c.value = bytes
                        gatt?.writeCharacteristic(c) == true
                    }
                } else {
                    val c = characteristic!!
                    if (Build.VERSION.SDK_INT >= 33) server?.notifyCharacteristicChanged(device, c, false, bytes) == BluetoothStatusCodes.SUCCESS
                    else { c.value = bytes; server?.notifyCharacteristicChanged(device, c, false) == true }
                }
                if (!accepted) { complete = null; done(false) }
            }
        }
    }

    private fun find(device: BluetoothDevice, central: Boolean) = links.values.firstOrNull { it.device.address == device.address && it.central == central }
    private fun find(gatt: BluetoothGatt) = links.values.firstOrNull { it.gatt === gatt }
    private fun reserve(device: BluetoothDevice, central: Boolean): Conn {
        if (links.size >= 6) links.values.minByOrNull { it.lastTraffic }?.let { close(it) }
        return Conn(device, central).also { c ->
            links[c.id] = c
            handler.postDelayed({ if (!c.opened) close(c) }, 15_000)
        }
    }

    private fun opened(c: Conn) {
        if (c.opened || c.closed) return
        c.opened = true
        val result = JSONObject(node.linkOpened(c.id, c.central, this))
        if (result.has("error")) { report(result.getString("error")); close(c) }
    }

    private fun close(c: Conn) {
        if (c.closed) return
        c.closed = true
        links.remove(c.id)
        c.ready.countDown()
        c.writer.close()
        c.complete?.invoke(false); c.complete = null
        node.linkClosed(c.id)
        if (c.central) { c.gatt?.disconnect(); c.gatt?.close() }
        else server?.cancelConnection(c.device)
    }

    private fun received(c: Conn?, bytes: ByteArray) {
        if (c == null || c.closed) return
        c.lastTraffic = SystemClock.elapsedRealtime()
        if (!node.linkReceived(c.id, bytes)) close(c)
    }

    private fun sent(c: Conn?, status: Int) {
        c ?: return
        c.lastTraffic = SystemClock.elapsedRealtime()
        c.complete?.invoke(status == BluetoothGatt.GATT_SUCCESS)
        c.complete = null
        if (status != BluetoothGatt.GATT_SUCCESS) close(c)
    }

    override fun write(handle: ULong, bytes: ByteArray): Boolean {
        val c = links[handle] ?: return false
        return c.ready.await(10, TimeUnit.SECONDS) && !c.closed && c.writer.write(bytes)
    }
    override fun linkClosed(handle: ULong) { handler.post { links[handle]?.let { close(it) } } }

    private val clientCallback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, state: Int) { handler.post {
            val c = find(g)
            if (c == null) { g.close(); return@post }
            if (status != BluetoothGatt.GATT_SUCCESS || state == BluetoothProfile.STATE_DISCONNECTED) close(c)
            else if (state == BluetoothProfile.STATE_CONNECTED && !c.discovering) {
                c.discovering = true
                g.requestConnectionPriority(BluetoothGatt.CONNECTION_PRIORITY_HIGH)
                g.setPreferredPhy(BluetoothDevice.PHY_LE_2M_MASK, BluetoothDevice.PHY_LE_2M_MASK, BluetoothDevice.PHY_OPTION_NO_PREFERRED)
                if (!g.discoverServices()) close(c)
            }
        } }
        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) { handler.post {
            val c = find(g) ?: return@post
            val ch = g.getService(SERVICE)?.getCharacteristic(STREAM)
            val desc = ch?.getDescriptor(CCCD)
            if (status != BluetoothGatt.GATT_SUCCESS || ch == null || desc == null) { close(c); return@post }
            c.remote = ch
            // Register before CCCD: the peripheral may send Hello as soon as it subscribes.
            opened(c)
            if (c.closed) return@post
            desc.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            if (!g.setCharacteristicNotification(ch, true) || !g.writeDescriptor(desc)) close(c)
        } }
        override fun onDescriptorWrite(g: BluetoothGatt, d: BluetoothGattDescriptor, status: Int) { handler.post {
            val c = find(g) ?: return@post
            if (status != BluetoothGatt.GATT_SUCCESS) { close(c); return@post }
            c.subscribed = true
            // Failure (or no callback) keeps the default MTU, never disconnects the link.
            if (!g.requestMtu(517)) c.ready.countDown()
            handler.postDelayed({ c.ready.countDown() }, 2_000)
        } }
        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) { handler.post {
            find(g)?.let { c ->
                if (status == BluetoothGatt.GATT_SUCCESS) c.mtu = mtu.coerceIn(23, 517)
                if (c.subscribed) c.ready.countDown()
            }
        } }
        override fun onCharacteristicWrite(g: BluetoothGatt, ch: BluetoothGattCharacteristic, status: Int) { handler.post { sent(find(g), status) } }
        override fun onCharacteristicChanged(g: BluetoothGatt, ch: BluetoothGattCharacteristic, value: ByteArray) { if (ch.uuid == STREAM) handler.post { received(find(g), value) } }
        override fun onCharacteristicChanged(g: BluetoothGatt, ch: BluetoothGattCharacteristic) {
            val bytes = ch.value?.clone() ?: return
            if (Build.VERSION.SDK_INT < 33 && ch.uuid == STREAM) handler.post { received(find(g), bytes) }
        }
    }

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onServiceAdded(status: Int, service: BluetoothGattService) { handler.post {
            if (status == BluetoothGatt.GATT_SUCCESS) advertise() else report("BLE service failed: $status; LAN remains active")
        } }
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, state: Int) { handler.post {
            if (stopped) return@post
            if (state == BluetoothProfile.STATE_CONNECTED && status == BluetoothGatt.GATT_SUCCESS) {
                if (find(device, false) == null) reserve(device, false)
            } else find(device, false)?.let { close(it) }
        } }
        override fun onMtuChanged(device: BluetoothDevice, mtu: Int) { handler.post { find(device, false)?.mtu = mtu.coerceIn(23, 517) } }
        override fun onDescriptorWriteRequest(device: BluetoothDevice, request: Int, d: BluetoothGattDescriptor, prepared: Boolean, response: Boolean, offset: Int, value: ByteArray) { handler.post {
            val c = find(device, false)
            val valid = c != null && d.uuid == CCCD && !prepared && offset == 0 && value.contentEquals(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
            if (response) server?.sendResponse(device, request, if (valid) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, 0, null)
            if (valid) { c!!.ready.countDown(); opened(c) } else if (c != null) close(c)
        } }
        override fun onCharacteristicWriteRequest(device: BluetoothDevice, request: Int, ch: BluetoothGattCharacteristic, prepared: Boolean, response: Boolean, offset: Int, value: ByteArray) { handler.post {
            val c = find(device, false)
            val valid = c?.opened == true && ch.uuid == STREAM && !prepared && offset == 0 && value.size <= c.mtu - 3
            if (response) server?.sendResponse(device, request, if (valid) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, 0, null)
            if (valid) received(c, value) else c?.let { close(it) }
        } }
        override fun onNotificationSent(device: BluetoothDevice, status: Int) { handler.post { sent(find(device, false), status) } }
    }

    private val scanCallback = object : ScanCallback() {
        override fun onScanResult(type: Int, result: ScanResult) { handler.post {
            if (stopped || links.values.any { it.device.address == result.device.address }) return@post
            val remote = result.scanRecord?.getServiceData(ParcelUuid(SERVICE)) ?: return@post
            if (remote.size != 4) return@post
            // Do not evict healthy peers on every scan result when six slots are full.
            if (links.size >= 6) return@post
            val c = reserve(result.device, true)
            c.gatt = result.device.connectGatt(context, false, clientCallback, BluetoothDevice.TRANSPORT_LE)
            if (c.gatt == null) close(c)
        } }
        override fun onScanFailed(error: Int) { handler.post { scanning = false; report("BLE scan failed: $error; LAN remains active") } }
    }
    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartFailure(error: Int) { handler.post { advertising = false; report("BLE advertise failed: $error; LAN remains active") } }
    }

    private fun advertise() {
        if (stopped || advertising) return
        // 128-bit service data + hint + flags fits 31 bytes. Put the UUID list in scan response.
        val data = AdvertiseData.Builder().addServiceData(ParcelUuid(SERVICE), hint).setIncludeDeviceName(false).build()
        val response = AdvertiseData.Builder().addServiceUuid(ParcelUuid(SERVICE)).build()
        val advertiser = adapter?.bluetoothLeAdvertiser
        if (advertiser == null) { report("BLE advertising unsupported; LAN remains active"); return }
        advertising = true
        advertiser.startAdvertising(AdvertiseSettings.Builder().setConnectable(true).setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY).build(), data, response, advertiseCallback)
    }

    private val scanCycle = object : Runnable {
        override fun run() {
            if (stopped) return
            if (!scanning) {
                val filter = ScanFilter.Builder().setServiceData(ParcelUuid(SERVICE), ByteArray(4), ByteArray(4)).build()
                adapter?.bluetoothLeScanner?.startScan(listOf(filter), ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(), scanCallback)
                scanning = true
                handler.postDelayed(this, if (foreground) 30_000 else 5_000)
            } else if (!foreground) {
                adapter?.bluetoothLeScanner?.stopScan(scanCallback); scanning = false
                handler.postDelayed(this, 15_000)
            } else handler.postDelayed(this, 30_000)
        }
    }

    fun setForeground(value: Boolean) { handler.post { foreground = value; handler.removeCallbacks(scanCycle); scanCycle.run() } }
    fun start() { handler.post {
        if (adapter?.isEnabled != true) { report("Enable Bluetooth, then restart mesh; LAN remains active"); return@post }
        server = manager.openGattServer(context, serverCallback)
        val service = BluetoothGattService(SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        characteristic = BluetoothGattCharacteristic(STREAM, BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE or BluetoothGattCharacteristic.PROPERTY_NOTIFY, BluetoothGattCharacteristic.PERMISSION_WRITE).apply {
            addDescriptor(BluetoothGattDescriptor(CCCD, BluetoothGattDescriptor.PERMISSION_WRITE))
        }
        service.addCharacteristic(characteristic)
        if (server?.addService(service) != true) report("BLE peripheral unavailable; LAN remains active")
        scanCycle.run()
    } }
    fun stop(done: () -> Unit) { handler.post {
        stopped = true
        handler.removeCallbacksAndMessages(null)
        if (scanning) adapter?.bluetoothLeScanner?.stopScan(scanCallback)
        if (advertising) adapter?.bluetoothLeAdvertiser?.stopAdvertising(advertiseCallback)
        links.values.toList().forEach { close(it) }
        server?.close(); server = null
        thread.quitSafely()
        done()
    } }
}

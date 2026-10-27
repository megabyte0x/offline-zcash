package org.zcash.offline

import android.app.*
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.*
import org.json.JSONObject
import uniffi.ozm.OfflineNode
import java.util.concurrent.Executors

class MeshService : Service() {
    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private lateinit var node: OfflineNode
    @Volatile private var ble: BleLink? = null
    private var multicast: WifiManager.MulticastLock? = null
    @Volatile var radioStatus = "Starting mesh"
        private set
    @Volatile private var visible = false
    @Volatile private var destroyed = false
    inner class LocalBinder : Binder() {
        fun call(action: (OfflineNode) -> String, result: (String) -> Unit) {
            if (destroyed) return
            worker.execute {
                val text = try { action(node) } catch (e: Exception) { JSONObject().put("error", e.message ?: "native call failed").toString() }
                main.post { result(text) }
            }
        }
        fun join(invite: String, result: (String) -> Unit) = call({ n ->
            val joined = n.join(invite)
            if (JSONObject(joined).has("error")) joined else startNode()
        }, result)
        fun start(result: (String) -> Unit) = call({ startNode() }, result)
        fun radio() = radioStatus
    }
    private val binder = LocalBinder()
    override fun onBind(intent: Intent) = binder

    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("mesh", "Offline mesh", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        startForeground(1, Notification.Builder(this, "mesh").setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
            .setContentTitle("Offline Zcash mesh").setContentText("Test coins only · relaying while running")
            .setContentIntent(open).setOngoing(true).build())
        worker.execute {
            node = OfflineNode(filesDir.resolve("ozm").absolutePath)
            if (JSONObject(node.status()).optBoolean("joined")) startNode() else radioStatus = "Import your device invite to join"
        }
    }

    private fun startNode(): String {
        val result = node.start("{}") // LAN discovery remains enabled, even on a Wi-Fi without internet.
        if (!JSONObject(result).has("error") && ble == null) {
            multicast = (applicationContext.getSystemService(WIFI_SERVICE) as WifiManager).createMulticastLock("ozm-discovery").apply { setReferenceCounted(false); acquire() }
            radioStatus = "BLE + LAN enabled · radio operation still experimental"
            ble = BleLink(this, node) { radioStatus = it }.also { it.start(); it.setForeground(visible) }
        }
        return result
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        visible = intent?.getBooleanExtra("visible", false) ?: false
        ble?.setForeground(visible)
        return START_STICKY
    }

    override fun onDestroy() {
        destroyed = true
        fun cleanup() { worker.execute {
            if (::node.isInitialized) { node.stop(); node.destroy() }
            multicast?.let { if (it.isHeld) it.release() }
            worker.shutdown()
        } }
        // Startup and teardown share the worker: do not destroy JNI while BLE still uses it.
        worker.execute {
            val b = ble
            if (b == null) cleanup() else b.stop { cleanup() }
        }
        super.onDestroy()
    }
}

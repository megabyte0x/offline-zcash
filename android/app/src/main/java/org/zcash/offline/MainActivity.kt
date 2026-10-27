package org.zcash.offline

import android.Manifest
import android.app.Activity
import android.content.*
import android.content.pm.PackageManager
import android.os.*
import android.text.InputType
import android.widget.*
import org.json.JSONObject
import java.math.BigDecimal

class MainActivity : Activity() {
    private var mesh: MeshService.LocalBinder? = null
    private var bound = false
    private lateinit var status: TextView
    private lateinit var neighbours: TextView
    private lateinit var payments: TextView
    private lateinit var result: TextView
    private val handler = Handler(Looper.getMainLooper())
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) { mesh = binder as MeshService.LocalBinder; refresh() }
        override fun onServiceDisconnected(name: ComponentName) { mesh = null }
    }
    private val poll = object : Runnable {
        override fun run() { refresh(); handler.postDelayed(this, 2_000) }
    }
    private fun requiredPermissions() = if (Build.VERSION.SDK_INT >= 31) arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_ADVERTISE) else arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
    private fun connect() {
        if (!requiredPermissions().all { checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }) {
            requestPermissions(requiredPermissions() + if (Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.POST_NOTIFICATIONS) else emptyArray(), 1)
            return
        }
        val intent = Intent(this, MeshService::class.java).putExtra("visible", true)
        startForegroundService(intent)
        if (!bound) { bound = bindService(intent, connection, BIND_AUTO_CREATE) }
    }
    override fun onRequestPermissionsResult(request: Int, permissions: Array<out String>, grants: IntArray) {
        super.onRequestPermissionsResult(request, permissions, grants)
        if (requiredPermissions().all { checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }) connect()
        else result.text = "Nearby devices (or Location on Android 10/11) permission is required. Use Start mesh to retry."
    }
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 32, 24, 32) }
        setContentView(ScrollView(this).apply { fitsSystemWindows = true; addView(column) })
        fun label(text: String, size: Float = 16f) = TextView(this).apply { this.text = text; textSize = size; setPadding(0, 12, 0, 12); setTextIsSelectable(true); column.addView(this) }
        fun button(text: String, action: () -> Unit) { column.addView(Button(this).apply { this.text = text; setOnClickListener { action() } }) }
        label("Offline Zcash · 0.5.0", 24f)
        label("Regtest only · no real ZEC\nKeep the mesh notification running to relay. Pending is not final; mined is gateway-reported.")
        status = label("Starting…")
        result = label("")
        button("Start mesh / grant permissions") { connect(); mesh?.start { showResult(it) } }
        button("Join with invite file") {
            if (mesh == null) { result.text = "Start mesh and grant permissions first" }
            else startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE), 2)
        }
        button("Sync from gateway") { mesh?.call({ it.syncNow() }, ::showResult) }
        button("Request 3 × 0.1 test ZEC") { mesh?.call({ it.requestFunds(3u, "0.1") }, ::showResult) }
        label("Pay a fleet device", 20f)
        val to = EditText(this).apply { hint = "Recipient name (case-sensitive)"; contentDescription = "Recipient name"; isSingleLine = true; column.addView(this) }
        val amount = EditText(this).apply { hint = "Amount in ZEC, e.g. 0.01"; contentDescription = "Amount in ZEC"; inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL; column.addView(this) }
        button("Build and send payment") {
            val m = mesh
            if (m == null) result.text = "Start mesh first" else {
                result.text = "Building proof on this device; this may take time…"
                val recipient = to.text.toString().trim()
                val value = amount.text.toString().trim()
                m.call({ it.pay(recipient, value) }, ::showResult)
            }
        }
        neighbours = label("Neighbours", 18f)
        payments = label("Payments", 18f)
        button("Stop mesh (stops relaying)") {
            if (bound) { unbindService(connection); bound = false }
            mesh = null; stopService(Intent(this, MeshService::class.java)); status.text = "Mesh stopped"
        }
        connect()
    }
    override fun onActivityResult(request: Int, code: Int, data: Intent?) {
        super.onActivityResult(request, code, data)
        if (request != 2 || code != RESULT_OK) return
        try {
            val uri = data?.data ?: return
            val text = contentResolver.openInputStream(uri)!!.use { input ->
                val bytes = ByteArray(1_048_577)
                var size = 0
                while (size < bytes.size) {
                    val n = input.read(bytes, size, bytes.size - size)
                    if (n < 0) break
                    size += n
                }
                require(size <= 1_048_576) { "Invite too large" }
                String(bytes, 0, size, Charsets.UTF_8)
            }
            mesh?.join(text, ::showResult)
        } catch (e: Exception) { result.text = "Cannot import invite: ${e.message}" }
    }
    private fun showResult(text: String) {
        val obj = JSONObject(text)
        result.text = if (obj.has("error")) obj.getString("error") else if (obj.has("txid")) "Payment ${obj.getString("txid")}\n${obj.optString("state")}" else "Request accepted. Watch status and payments below."
        refresh()
    }
    private fun refresh() {
        val m = mesh ?: return
        m.call({ n -> JSONObject().put("status", JSONObject(n.status())).put("payments", JSONObject(n.listPayments())).put("fleet", JSONObject(n.fleet())).toString() }) { text ->
            val obj = JSONObject(text)
            if (obj.has("error")) { result.text = obj.getString("error"); return@call }
            val s = obj.getJSONObject("status")
            val wallet = s.optJSONObject("wallet")
            fun zec(z: Long) = BigDecimal.valueOf(z, 8).stripTrailingZeros().toPlainString()
            status.text = if (!s.optBoolean("joined")) "Not joined · import your invite file" else "${s.optString("name")} (${s.optString("role")}) · ${if (s.optBoolean("running")) "running" else "stopped"}\nTip ${wallet?.optLong("tip") ?: 0} · spendable ${zec(wallet?.optLong("spendable") ?: 0)} ZEC\n${m.radio()}"
            val links = s.optJSONArray("neighbor_links")
            val devices = obj.getJSONObject("fleet").optJSONArray("devices")
            val peers = if (links == null || links.length() == 0) "None yet" else (0 until links.length()).joinToString("\n") { i ->
                val link = links.getJSONObject(i)
                "${link.getString("name")} · ${link.getString("kind").uppercase()}"
            }
            val names = if (devices == null) "join first" else (0 until devices.length()).joinToString(", ") { devices.getJSONObject(it).getString("name") }
            neighbours.text = "Neighbours\n$peers\n\nFleet: $names"
            val list = obj.getJSONObject("payments").optJSONArray("payments")
            payments.text = "Payments\n" + if (list == null || list.length() == 0) "No payments" else (0 until list.length()).joinToString("\n\n") { i ->
                val p = list.getJSONObject(i)
                "${p.getString("direction")} · ${p.getString("counterparty")} · ${zec(p.getLong("amount"))} ZEC\n${p.getString("state")}\n${p.getString("txid")}"
            }
        }
    }
    override fun onResume() { super.onResume(); if (bound) startService(Intent(this, MeshService::class.java).putExtra("visible", true)); handler.post(poll) }
    override fun onPause() { handler.removeCallbacks(poll); if (bound) startService(Intent(this, MeshService::class.java).putExtra("visible", false)); super.onPause() }
    override fun onDestroy() { if (bound) unbindService(connection); super.onDestroy() }
}

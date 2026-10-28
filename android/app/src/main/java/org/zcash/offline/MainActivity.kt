package org.zcash.offline

import android.Manifest
import android.app.Activity
import android.content.*
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Typeface
import android.os.*
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.View
import android.widget.*
import org.json.JSONObject
import java.math.BigDecimal

class MainActivity : Activity() {
    private var mesh: MeshService.LocalBinder? = null
    private var bound = false
    private var pendingStart = false
    private var selectedNetwork = "regtest"
    private var network = "regtest"
    private lateinit var status: TextView
    private lateinit var chain: TextView
    private lateinit var warning: TextView
    private lateinit var neighbours: TextView
    private lateinit var payments: TextView
    private lateinit var result: TextView
    private lateinit var address: TextView
    private lateinit var fundButton: Button
    private lateinit var faucetNote: TextView
    private lateinit var toHint: TextView
    private lateinit var to: EditText
    private var ownAddress = ""
    private val handler = Handler(Looper.getMainLooper())
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            mesh = binder as MeshService.LocalBinder
            if (pendingStart) { pendingStart = false; mesh?.startOn(selectedNetwork, ::showResult) }
            refresh()
        }
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
        else say("Nearby devices permission (Location on Android 10/11) is required for the mesh. Tap Start to ask again.", error = true)
    }
    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
    private fun say(text: String, error: Boolean = false) {
        result.text = text
        result.setTextColor(if (error) BAD else Color.DKGRAY)
    }
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(16), dp(16), dp(16), dp(32)) }
        setContentView(ScrollView(this).apply { fitsSystemWindows = true; addView(column) })
        fun label(text: String, size: Float = 15f, muted: Boolean = false) = TextView(this).apply {
            this.text = text; textSize = size; setPadding(0, dp(4), 0, dp(4)); setTextIsSelectable(true)
            if (muted) setTextColor(Color.DKGRAY)
            column.addView(this)
        }
        fun heading(text: String) = TextView(this).apply {
            this.text = text; textSize = 19f; setTypeface(typeface, Typeface.BOLD); setPadding(0, dp(20), 0, dp(4))
            isAccessibilityHeading = true; column.addView(this)
        }
        fun button(text: String, action: () -> Unit) = Button(this).apply { this.text = text; setOnClickListener { action() }; column.addView(this) }
        fun field(caption: String, hint: String, type: Int, value: String = ""): EditText {
            val edit = EditText(this).apply { id = View.generateViewId(); this.hint = hint; inputType = type; setText(value); isSingleLine = true }
            column.addView(TextView(this).apply { text = caption; textSize = 14f; setPadding(0, dp(8), 0, 0); labelFor = edit.id })
            column.addView(edit)
            return edit
        }
        val decimal = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL

        label("Offline Zcash · 0.7.3", 24f).setTypeface(null, Typeface.BOLD)
        label("Test coins only · Regtest or public testnet. Included and mined come from the gateway; this app does not verify them independently.", 14f, muted = true)
        result = label("").apply { accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE; textSize = 15f }

        heading("Status")
        status = label("Starting…")
        chain = label("", 14f, muted = true)
        warning = label("", 14f).apply { setTextColor(WARN); visibility = View.GONE }

        heading("Get started")
        label("No invite needed: Start creates your keys on this phone and finds a gateway nearby. An organiser invite is optional.", 14f, muted = true)
        val networkSpinner = Spinner(this).apply {
            id = View.generateViewId()
            adapter = ArrayAdapter(this@MainActivity, android.R.layout.simple_spinner_dropdown_item, arrayOf("regtest", "testnet"))
            contentDescription = "Network for new identity"
        }
        column.addView(TextView(this).apply { text = "Network (new identity only)"; textSize = 14f; labelFor = networkSpinner.id })
        column.addView(networkSpinner)
        button("Start without invite") {
            selectedNetwork = networkSpinner.selectedItem.toString()
            pendingStart = true
            say("Starting the mesh…")
            connect()
            mesh?.let { pendingStart = false; it.startOn(selectedNetwork, ::showResult) }
        }
        button("Join with invite file") {
            if (mesh == null) say("Tap Start first and allow Nearby devices, then import the invite.", error = true)
            else startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE), 2)
        }

        heading("Wallet")
        button("Sync now") { say("Syncing…"); mesh?.call({ it.syncNow() }, ::showResult) ?: say("Start the mesh first.", error = true) }
        fundButton = button("Get test funds (3 × 0.1 ZEC)") { say("Asking the gateway for test funds…"); mesh?.call({ it.requestFunds(3u, "0.1") }, ::showResult) ?: say("Start the mesh first.", error = true) }
        faucetNote = label("Testnet funds: faucet.testnet.valargroup.dev (0.125 TAZ / 24 h; the service may fail). Gateways do not hand out testnet funds.", 14f, muted = true).apply { visibility = View.GONE }

        heading("Pay a Zcash address")
        to = field("Recipient address", "uregtest1…", InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS)
        toHint = label("Paste the unified address the recipient shared with you.", 13f, muted = true)
        to.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) = checkAddress()
        })
        val amount = field("Amount (ZEC)", "0.01", decimal, "0.01")
        val fee = label("", 13f, muted = true)
        button("Max") {
            val m = mesh ?: return@button say("Start the mesh first.", error = true)
            m.call({ it.maxPayment() }) { text ->
                val quote = JSONObject(text)
                if (quote.has("error")) say("Cannot compute Max: ${quote.getString("error")}", error = true) else {
                    amount.setText(quote.getString("amount"))
                    fee.text = "Max after the builder fee of ${BigDecimal.valueOf(quote.getLong("fee_zat"), 8).stripTrailingZeros().toPlainString()} ZEC. Changes if your spendable notes change."
                }
            }
        }
        button("Build, prove and send") {
            val m = mesh ?: return@button say("Start the mesh first.", error = true)
            val recipient = to.text.toString().trim()
            val value = amount.text.toString().trim()
            when {
                recipient.isEmpty() -> to.error = "Enter the recipient's address"
                !AMOUNT.matches(value) || value.toBigDecimal().signum() <= 0 -> amount.error = "Enter an amount like 0.01 (up to 8 decimals)"
                else -> { say("Building and proving on this phone. This can take a while…"); m.call({ it.pay(recipient, value) }, ::showResult) }
            }
        }
        label("A known mesh payee answers with a receipt. An unknown address is still submitted, but you get no payee receipt.", 13f, muted = true)

        heading("Receive")
        address = label("No address yet.", 13f).apply { typeface = Typeface.MONOSPACE }
        button("Copy your address") {
            if (ownAddress.isEmpty()) say("No address yet: start the mesh first.", error = true) else {
                (getSystemService(CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Zcash address", ownAddress))
                say("Address copied.")
            }
        }
        label("Open or unknown payers are not accepted while pending. Wait for gateway-reported inclusion.", 13f, muted = true)

        heading("Prepare for offline")
        label("Split one mined note into equal notes so you can pay several times without a connection.\n1. Prepare while a gateway is reachable.\n2. Wait until the split shows Mined (10 confirmations on testnet).\n3. Sync, then go offline. Never spend unmined change.", 14f, muted = true)
        val notes = field("Number of notes", "10", InputType.TYPE_CLASS_NUMBER, "10")
        val noteValue = field("ZEC per note", "0.0101", decimal, "0.0101")
        label("The split fee grows with the number of notes.", 13f, muted = true)
        button("Prepare for offline") {
            val m = mesh ?: return@button say("Start the mesh first.", error = true)
            val count = notes.text.toString().toUIntOrNull()
            val value = noteValue.text.toString().trim()
            when {
                count == null || count < 2u || count > 100u -> notes.error = "Choose between 2 and 100 notes"
                !AMOUNT.matches(value) || value.toBigDecimal().signum() <= 0 -> noteValue.error = "Enter an amount like 0.0101"
                else -> { say("Proving the split…"); m.call({ it.prepareOffline(count, value) }, ::showResult) }
            }
        }

        heading("Payments")
        payments = label("Start the mesh to see payments.", 14f)
        heading("Mesh")
        neighbours = label("No neighbours yet.", 14f)
        button("Stop mesh (stops relaying)") {
            if (bound) { unbindService(connection); bound = false }
            mesh = null; stopService(Intent(this, MeshService::class.java)); status.text = "Mesh stopped. Tap Start to resume."
        }
        connect()
    }
    private fun checkAddress() {
        val want = PREFIX[network] ?: "u1"
        val v = to.text.toString().trim()
        val wrong = v.isNotEmpty() && !v.startsWith(want)
        toHint.text = if (wrong) "This does not look like a $network unified address (expected $want…)." else "Paste the unified address the recipient shared with you."
        toHint.setTextColor(if (wrong) WARN else Color.DKGRAY)
        to.hint = "$want…"
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
            say("Joining with the invite…")
            mesh?.join(text, ::showResult)
        } catch (e: Exception) { say("Cannot import invite: ${e.message}", error = true) }
    }
    private fun showResult(text: String) {
        val obj = JSONObject(text)
        when {
            obj.has("error") -> say("Not done: ${obj.getString("error")}", error = true)
            obj.has("txid") -> say("${if (obj.optString("direction") == "prepare") "Split" else "Payment"} ${obj.getString("txid").take(16)}… · ${stateLabel(obj.optString("state"))}. Follow it under Payments.")
            obj.has("requested_from") -> say("Asked ${obj.getString("requested_from")}; the wallet updates when the gateway answers.")
            obj.has("tip") -> say("Synced to height ${obj.get("tip")}.")
            else -> say("Done. Watch status and payments below.")
        }
        refresh()
    }
    private fun refresh() {
        val m = mesh ?: return
        m.call({ n -> JSONObject().put("status", JSONObject(n.status())).put("payments", JSONObject(n.listPayments())).toString() }) { text ->
            val obj = JSONObject(text)
            if (obj.has("error")) { status.text = "Node not ready: ${obj.getString("error")}\n${m.radio()}"; return@call }
            val s = obj.getJSONObject("status")
            val wallet = s.optJSONObject("wallet")
            fun zec(z: Long) = BigDecimal.valueOf(z, 8).stripTrailingZeros().toPlainString()
            network = s.optString("network", "regtest")
            ownAddress = s.optString("address")
            address.text = ownAddress.ifEmpty { "No address yet." }
            fundButton.visibility = if (network == "testnet" || s.optBoolean("online")) View.GONE else View.VISIBLE
            faucetNote.visibility = if (network == "testnet") View.VISIBLE else View.GONE
            checkAddress()
            val running = s.optBoolean("running")
            status.text = if (s.optString("name").isEmpty()) "Not started · ${m.radio()}" else
                listOf(s.optString("name"), s.optString("role"), network, if (running) "running" else "stopped").filter { it.isNotEmpty() }.joinToString(" · ") + "\n" +
                "${zec(wallet?.optLong("spendable") ?: 0)} ZEC spendable · ${zec(wallet?.optLong("locked") ?: 0)} ZEC locked · synced to height ${wallet?.optLong("tip") ?: 0}\n${m.radio()}"
            val gateway = s.optJSONObject("selected_gateway")
            val sources = s.optJSONArray("chain_sources")?.length() ?: 0
            chain.text = "Chain view (gateway-reported): " +
                (if (gateway == null) (if (s.optBoolean("online")) "this device is the gateway." else "finding a gateway…")
                 else "gateway ${gateway.optString("name")} · ${gateway.optInt("hops")} hops · tip ${gateway.optLong("tip")} · mesh ID ${gateway.optString("peer_id")}.") +
                (if (!s.optBoolean("single_source", true)) " Cross-checked against $sources chain sources." else if (sources > 0) " Single chain source: not cross-checked." else " No chain source connected yet.")
            val warn = s.optString("gateway_warning")
            warning.text = warn
            warning.visibility = if (warn.isEmpty()) View.GONE else View.VISIBLE
            val links = s.optJSONArray("neighbor_links")
            neighbours.text = "Direct neighbours: " + if (links == null || links.length() == 0) "none yet" else (0 until links.length()).joinToString(", ") { i ->
                val link = links.getJSONObject(i)
                "${link.getString("name")} (${link.getString("kind").uppercase()})"
            }
            val list = obj.getJSONObject("payments").optJSONArray("payments")
            val refusals = obj.getJSONObject("payments").optJSONArray("refusals")
            payments.text = (if (list == null || list.length() == 0) "No payments yet." else (0 until list.length()).map { list.getJSONObject(it) }
                .sortedByDescending { it.optLong("created_ms") }.joinToString("\n\n") { p ->
                    val direction = when (p.getString("direction")) { "sent" -> "Sent"; "received" -> "Received"; "prepare" -> "Prepare (split to self)"; else -> p.getString("direction") }
                    val state = p.getString("state")
                    "${stateLabel(state)}\n$direction · ${zec(p.getLong("amount"))} ZEC · with ${p.getString("counterparty")}" +
                        (STATES[state]?.second?.let { "\n$it" } ?: "") +
                        (if (p.getString("direction") == "sent" && p.isNull("receipt_peer") && state != "submitted, no payee receipt") "\nNo payee receipt" else "") +
                        "\n${p.getString("txid")}"
                }) + if (refusals == null || refusals.length() == 0) "" else "\n\n" + (0 until refusals.length()).joinToString("\n") { i ->
                    val r = refusals.getJSONObject(i)
                    val reject = r.optJSONObject("reject")
                    "Refused by ${r.optString("from")}: code ${reject?.optInt("code")}, ${reject?.optString("reason")}. It may still be mined until it expires."
                }
        }
    }
    override fun onResume() { super.onResume(); if (bound) startService(Intent(this, MeshService::class.java).putExtra("visible", true)); handler.post(poll) }
    override fun onPause() { handler.removeCallbacks(poll); if (bound) startService(Intent(this, MeshService::class.java).putExtra("visible", false)); super.onPause() }
    override fun onDestroy() { if (bound) unbindService(connection); super.onDestroy() }

    companion object {
        private val BAD = Color.rgb(0xB4, 0x23, 0x18)
        private val WARN = Color.rgb(0x8A, 0x5A, 0x00)
        private val AMOUNT = Regex("""\d+(\.\d{1,8})?""")
        private val PREFIX = mapOf("regtest" to "uregtest1", "testnet" to "utest1", "mainnet" to "u1")
        // Label and explanation per payment state; the same table as the desktop apps. Chain results stay gateway-reported.
        private val STATES = mapOf(
            "built" to ("Built" to "Proof built on this phone; not sent yet."),
            "sent" to ("Sent over the mesh" to "Waiting for the payee device to receive it."),
            "delivered" to ("Delivered" to "The payee device received it; waiting for its receipt."),
            "accepted-pending" to ("Accepted, pending" to "An invited payee accepted it before inclusion. It is not on chain yet."),
            "awaiting-inclusion" to ("Awaiting inclusion" to "Checked on this phone; waiting for the gateway to report inclusion."),
            "submitted" to ("Submitted" to "A gateway submitted it; not included yet."),
            "submitted, no payee receipt" to ("Submitted · no payee receipt" to "Sent to a gateway for an address with no mesh payee. No receipt will come."),
            "in-mempool" to ("In mempool" to "In the gateway's mempool (gateway-reported)."),
            "retrying" to ("Retrying" to "The gateway hit a retryable error and will resubmit."),
            "included" to ("Included · gateway-reported" to "The gateway reports fewer than 10 confirmations. Not independently proved."),
            "mined" to ("Mined · gateway-reported" to "The gateway reports it mined. Not independently proved."),
            "expired" to ("Expired" to "The gateway reports it expired. Locked notes return after your sync passes the expiry height."),
            "conflict" to ("Conflict" to "The gateway reports another transaction spending the same note."),
            "gateway-rejected" to ("Rejected by gateway" to "The gateway refused to submit it."),
            "refused, may still mine until expiry" to ("Refused by payee" to "The payee refused it. It may still be mined until it expires."),
        )
        fun stateLabel(state: String) = STATES[state]?.first ?: state
    }
}

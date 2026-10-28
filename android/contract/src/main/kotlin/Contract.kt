import org.zcash.offline.ChunkWriter
import org.json.JSONObject
import uniffi.ozm.LinkSink
import uniffi.ozm.OfflineNode
import java.io.File
import java.net.URI
import java.net.http.*
import java.util.concurrent.*
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

fun checked(text: String): JSONObject = JSONObject(text).also { check(!it.has("error")) { it.toString() } }
fun until(what: String, seconds: Long = 180, condition: () -> Boolean) {
    val end = System.nanoTime() + TimeUnit.SECONDS.toNanos(seconds)
    while (!condition()) { check(System.nanoTime() < end) { "Timed out: $what" }; Thread.sleep(50) }
}
fun payment(node: OfflineNode, txid: String): JSONObject? {
    val ps = checked(node.listPayments()).getJSONArray("payments")
    return (0 until ps.length()).map { ps.getJSONObject(it) }.singleOrNull { it.getString("txid") == txid }
}

private class Wire(val a: OfflineNode, val b: OfflineNode, val id: ULong, val chunk: Int, val executor: ScheduledExecutorService) {
    val gate = CountDownLatch(1)
    val chunks = AtomicLong()
    val bytes = AtomicLong()
    val closed = CountDownLatch(2)
    val sinks = listOf(Sink(b), Sink(a))
    inner class Sink(val destination: OfflineNode) : LinkSink {
        val inFlight = AtomicInteger()
        val writer = ChunkWriter({ chunk }) { data, done ->
            check(inFlight.incrementAndGet() == 1) { "platform queue overflow" }
            check(data.size <= chunk)
            val count = chunks.incrementAndGet()
            bytes.addAndGet(data.size.toLong())
            executor.schedule({
                val ok = destination.linkReceived(id, data)
                inFlight.decrementAndGet()
                done(ok)
            }, if (count % 11L == 0L) 4 else 0, TimeUnit.MILLISECONDS)
        }
        override fun write(handle: ULong, bytes: ByteArray): Boolean = gate.await(5, TimeUnit.SECONDS) && writer.write(bytes)
        override fun linkClosed(handle: ULong) { writer.close(); destination.linkClosed(id); closed.countDown() }
    }
    fun open() {
        checked(a.linkOpened(id, true, sinks[0]))
        checked(b.linkOpened(id, false, sinks[1]))
        gate.countDown()
    }
    fun close() { a.linkClosed(id); b.linkClosed(id); check(closed.await(10, TimeUnit.SECONDS)) { "close callback missing" } }
}

fun main(args: Array<String>) {
    require(args.size == 2) { "ContractKt <state-directory> <rpc-url>" }
    val root = File(args[0])
    val rpc = args[1]
    val executor = Executors.newScheduledThreadPool(2)
    val nodes = listOf("A", "B", "D").map { name ->
        OfflineNode(File(root, name).absolutePath).also { n ->
            check(!checked(n.status()).getBoolean("joined"))
            checked(n.join(File(root, "invites/$name.invite.json").readText()))
            val options = JSONObject().put("listen", "127.0.0.1:0").put("discover", false)
            if (name == "D") options.put("rpc", rpc).put("mine", true)
            checked(n.start(options.toString()))
            check(checked(n.status()).getJSONArray("neighbors").length() == 0)
        }
    }
    val (a, b, d) = nodes
    try {
        // Flow-control rejection and cancellation must not claim successful delivery.
        check(!ChunkWriter({ 20 }) { _, done -> done(false) }.write(ByteArray(50)))
        val blocked = CountDownLatch(1)
        val stalled = ChunkWriter({ 20 }) { _, _ -> blocked.countDown() }
        val write = executor.submit<Boolean> { stalled.write(ByteArray(50)) }
        check(blocked.await(2, TimeUnit.SECONDS)); stalled.close(); check(!write.get(2, TimeUnit.SECONDS))
        for ((index, chunk) in listOf(20, 182, 514).withIndex()) {
            val ab = Wire(a, b, (index * 2 + 1).toULong(), chunk, executor)
            val bd = Wire(b, d, (index * 2 + 2).toULong(), chunk, executor)
            ab.open(); bd.open()
            until("A-B-D via Kotlin only") {
                listOf(a, b, d).map { checked(it.status()).getJSONArray("neighbors").length() } == listOf(1, 2, 1)
            }
            for (n in nodes) {
                val links = checked(n.status()).getJSONArray("neighbor_links")
                check((0 until links.length()).all { links.getJSONObject(it).getString("kind") == "ble" })
            }
            checked(d.syncNow()); checked(a.syncNow()); checked(b.syncNow())
            until("B synced via Kotlin") { checked(b.status()).getJSONObject("wallet").getLong("tip") > 0 }
            if (index == 0) {
                checked(a.requestFunds(3u, "0.1"))
                until("A funded via Kotlin") { checked(a.status()).getJSONObject("wallet").getLong("spendable") >= 30_000_000 }
            }
            val built = checked(a.pay("B", "0.037"))
            val txid = built.getString("txid")
            until("mined at A and B") { payment(a, txid)?.getString("state") == "mined" && payment(b, txid)?.getString("state") == "mined" }
            check(payment(b, txid)!!.getLong("amount") == 3_700_000L)
            check(!payment(b, txid)!!.isNull("receipt"))
            val response = HttpClient.newHttpClient().send(HttpRequest.newBuilder(URI(rpc)).header("Content-Type", "application/json").POST(HttpRequest.BodyPublishers.ofString(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("method", "getrawtransaction").put("params", org.json.JSONArray().put(txid).put(1)).toString())).build(), HttpResponse.BodyHandlers.ofString())
            val chain = JSONObject(response.body()).getJSONObject("result")
            check(chain.getLong("height") > 0)
            println("PASS Kotlin JNI chunk=$chunk A-B-D only: txid=$txid mined height=${chain.getLong("height")}; receipt persisted; chunks=${ab.chunks.get() + bd.chunks.get()} bytes=${ab.bytes.get() + bd.bytes.get()}")
            ab.close(); bd.close()
            until("no neighbours after link_closed", 10) { nodes.all { checked(it.status()).getJSONArray("neighbors").length() == 0 } }
        }
        check(checked(a.listPayments()).getJSONArray("payments").length() == 3)
        check(checked(b.listPayments()).getJSONArray("payments").length() == 3)
        println("PASS close callbacks, reconnect inventory, one payment per txid, callback backpressure/rejection/cancellation; no TCP or LAN connections")
    } finally {
        nodes.forEach { it.stop() }
        nodes.forEach { it.destroy() }
        executor.shutdownNow()
    }
}

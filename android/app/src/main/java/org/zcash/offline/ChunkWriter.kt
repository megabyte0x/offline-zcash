package org.zcash.offline

import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit

/** One in-flight ATT chunk, no unbounded platform queue. The callback is local flow control,
 * not an end-to-end acknowledgement; mesh acks and reconnect inventory provide that. */
class ChunkWriter(private val payloadSize: () -> Int, private val send: (ByteArray, (Boolean) -> Unit) -> Unit) {
    @Volatile private var closed = false
    @Volatile private var pending: CompletableFuture<Boolean>? = null

    @Synchronized fun write(bytes: ByteArray): Boolean {
        var offset = 0
        while (offset < bytes.size && !closed) {
            val end = minOf(bytes.size, offset + payloadSize().coerceIn(20, 514))
            val done = CompletableFuture<Boolean>()
            pending = done
            if (closed) return false
            try {
                send(bytes.copyOfRange(offset, end)) { done.complete(it) }
                if (!done.get(15, TimeUnit.SECONDS)) return false
            } catch (_: Exception) {
                return false
            } finally { pending = null }
            offset = end
        }
        return !closed && offset == bytes.size
    }

    fun close() { closed = true; pending?.complete(false) }
}

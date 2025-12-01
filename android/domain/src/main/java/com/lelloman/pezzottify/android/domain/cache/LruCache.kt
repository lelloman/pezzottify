package com.lelloman.pezzottify.android.domain.cache

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

class LruCache<K, V>(
    private val maxEntries: () -> Int,
    private val maxSizeBytes: () -> Long,
    private val ttlMillis: Long,
    private val sizeCalculator: (V) -> Int,
    private val timeProvider: () -> Long = { System.currentTimeMillis() }
) {
    private val cache = ConcurrentHashMap<K, CacheEntry<V>>()
    private val lock = ReentrantReadWriteLock()

    // Metrics
    private var hits = 0L
    private var misses = 0L
    private var evictions = 0L
    private var expirations = 0L

    fun get(key: K): V? = lock.read {
        val entry = cache[key] ?: run {
            misses++
            return null
        }

        val now = timeProvider()
        if (entry.isExpired(ttlMillis, now)) {
            cache.remove(key)
            expirations++
            misses++
            return null
        }

        // Update last accessed time (LRU tracking)
        cache[key] = entry.touch(now)
        hits++
        return entry.value
    }

    fun put(key: K, value: V) = lock.write {
        val now = timeProvider()
        val size = sizeCalculator(value)

        // Remove expired entries first
        evictExpired(now)

        // Evict if needed for size constraints
        evictIfNeeded(size)

        cache[key] = CacheEntry(
            value = value,
            createdAt = now,
            lastAccessedAt = now,
            sizeBytes = size
        )
    }

    fun remove(key: K) = lock.write {
        cache.remove(key)
    }

    fun clear() = lock.write {
        cache.clear()
    }

    fun getMetrics(): CacheMetrics {
        return CacheMetrics(
            hits = hits,
            misses = misses,
            evictions = evictions,
            expirations = expirations,
            currentEntries = cache.size,
            currentSizeBytes = cache.values.sumOf { it.sizeBytes.toLong() },
            hitRate = if (hits + misses > 0) hits.toDouble() / (hits + misses) else 0.0
        )
    }

    fun resetMetrics() {
        hits = 0
        misses = 0
        evictions = 0
        expirations = 0
    }

    private fun evictExpired(now: Long) {
        val expired = cache.entries.filter { it.value.isExpired(ttlMillis, now) }
        expired.forEach {
            cache.remove(it.key)
            expirations++
        }
    }

    private fun evictIfNeeded(incomingSize: Int) {
        val maxEntriesNow = maxEntries()
        val maxBytesNow = maxSizeBytes()

        // Evict by entry count
        while (cache.size >= maxEntriesNow && cache.isNotEmpty()) {
            evictLru()
        }

        // Evict by size
        var currentSize = cache.values.sumOf { it.sizeBytes.toLong() }
        while (currentSize + incomingSize > maxBytesNow && cache.isNotEmpty()) {
            evictLru()
            currentSize = cache.values.sumOf { it.sizeBytes.toLong() }
        }
    }

    private fun evictLru() {
        val lruKey = cache.entries
            .minByOrNull { it.value.lastAccessedAt }
            ?.key

        if (lruKey != null) {
            cache.remove(lruKey)
            evictions++
        }
    }
}

data class CacheMetrics(
    val hits: Long,
    val misses: Long,
    val evictions: Long,
    val expirations: Long,
    val currentEntries: Int,
    val currentSizeBytes: Long,
    val hitRate: Double
)

package com.lelloman.pezzottify.android.domain.cache

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class LruCacheTest {

    @Test
    fun `get returns null for missing key`() {
        val cache = createCache()

        val result = cache.get("missing-key")

        assertThat(result).isNull()
    }

    @Test
    fun `put and get returns stored value`() {
        val cache = createCache()

        cache.put("key1", "value1")
        val result = cache.get("key1")

        assertThat(result).isEqualTo("value1")
    }

    @Test
    fun `put overwrites existing value`() {
        val cache = createCache()

        cache.put("key1", "original")
        cache.put("key1", "updated")
        val result = cache.get("key1")

        assertThat(result).isEqualTo("updated")
    }

    @Test
    fun `remove deletes entry`() {
        val cache = createCache()
        cache.put("key1", "value1")

        cache.remove("key1")
        val result = cache.get("key1")

        assertThat(result).isNull()
    }

    @Test
    fun `clear removes all entries`() {
        val cache = createCache()
        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.put("key3", "value3")

        cache.clear()

        assertThat(cache.get("key1")).isNull()
        assertThat(cache.get("key2")).isNull()
        assertThat(cache.get("key3")).isNull()
    }

    // TTL Expiration Tests

    @Test
    fun `get returns null for expired entry`() {
        var currentTime = 1000L
        val cache = createCache(
            ttlMillis = 5000L,
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        currentTime += 6000L // Advance past TTL

        val result = cache.get("key1")

        assertThat(result).isNull()
    }

    @Test
    fun `get returns value for non-expired entry`() {
        var currentTime = 1000L
        val cache = createCache(
            ttlMillis = 5000L,
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        currentTime += 4000L // Still within TTL

        val result = cache.get("key1")

        assertThat(result).isEqualTo("value1")
    }

    @Test
    fun `expired entries are removed on put`() {
        var currentTime = 1000L
        val cache = createCache(
            ttlMillis = 5000L,
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        currentTime += 6000L // Expire key1
        cache.put("key2", "value2") // This should trigger cleanup

        val metrics = cache.getMetrics()
        assertThat(metrics.expirations).isEqualTo(1)
    }

    // LRU Eviction Tests

    @Test
    fun `evicts LRU entry when max entries exceeded`() {
        val cache = createCache(maxEntries = 3)

        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.put("key3", "value3")
        cache.put("key4", "value4") // Should evict key1 (LRU)

        assertThat(cache.get("key1")).isNull()
        assertThat(cache.get("key2")).isEqualTo("value2")
        assertThat(cache.get("key3")).isEqualTo("value3")
        assertThat(cache.get("key4")).isEqualTo("value4")
    }

    @Test
    fun `accessing entry updates LRU order`() {
        var currentTime = 1000L
        val cache = createCache(
            maxEntries = 3,
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        currentTime += 100
        cache.put("key2", "value2")
        currentTime += 100
        cache.put("key3", "value3")
        currentTime += 100

        // Access key1 to make it recently used
        cache.get("key1")
        currentTime += 100

        // Now add key4 - should evict key2 (LRU after key1 access)
        cache.put("key4", "value4")

        assertThat(cache.get("key1")).isEqualTo("value1")
        assertThat(cache.get("key2")).isNull() // Evicted
        assertThat(cache.get("key3")).isEqualTo("value3")
        assertThat(cache.get("key4")).isEqualTo("value4")
    }

    @Test
    fun `evicts entries when max size exceeded`() {
        val cache = createCache(
            maxEntries = 100,
            maxSizeBytes = 50L,
            sizeCalculator = { it.length } // Size = string length
        )

        cache.put("key1", "12345678901234567890") // 20 bytes
        cache.put("key2", "12345678901234567890") // 20 bytes
        cache.put("key3", "12345678901234567890") // 20 bytes - exceeds 50, should evict key1

        assertThat(cache.get("key1")).isNull()
        assertThat(cache.get("key2")).isNotNull()
        assertThat(cache.get("key3")).isNotNull()
    }

    // Metrics Tests

    @Test
    fun `metrics tracks hits correctly`() {
        val cache = createCache()
        cache.put("key1", "value1")

        cache.get("key1")
        cache.get("key1")
        cache.get("key1")

        val metrics = cache.getMetrics()
        assertThat(metrics.hits).isEqualTo(3)
    }

    @Test
    fun `metrics tracks misses correctly`() {
        val cache = createCache()

        cache.get("missing1")
        cache.get("missing2")

        val metrics = cache.getMetrics()
        assertThat(metrics.misses).isEqualTo(2)
    }

    @Test
    fun `metrics tracks evictions correctly`() {
        val cache = createCache(maxEntries = 2)

        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.put("key3", "value3") // Evicts key1
        cache.put("key4", "value4") // Evicts key2

        val metrics = cache.getMetrics()
        assertThat(metrics.evictions).isEqualTo(2)
    }

    @Test
    fun `metrics tracks expirations correctly`() {
        var currentTime = 1000L
        val cache = createCache(
            ttlMillis = 5000L,
            timeProvider = { currentTime }
        )

        cache.put("key1", "value1")
        cache.put("key2", "value2")
        currentTime += 6000L // Expire both

        cache.get("key1") // Triggers expiration check
        cache.get("key2") // Triggers expiration check

        val metrics = cache.getMetrics()
        assertThat(metrics.expirations).isEqualTo(2)
    }

    @Test
    fun `metrics calculates hit rate correctly`() {
        val cache = createCache()
        cache.put("key1", "value1")

        cache.get("key1") // Hit
        cache.get("key1") // Hit
        cache.get("key1") // Hit
        cache.get("missing") // Miss

        val metrics = cache.getMetrics()
        assertThat(metrics.hitRate).isWithin(0.001).of(0.75) // 3/4 = 75%
    }

    @Test
    fun `metrics tracks current entries count`() {
        val cache = createCache()

        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.put("key3", "value3")

        val metrics = cache.getMetrics()
        assertThat(metrics.currentEntries).isEqualTo(3)
    }

    @Test
    fun `metrics tracks current size in bytes`() {
        val cache = createCache(sizeCalculator = { it.length })

        cache.put("key1", "12345") // 5 bytes
        cache.put("key2", "1234567890") // 10 bytes

        val metrics = cache.getMetrics()
        assertThat(metrics.currentSizeBytes).isEqualTo(15)
    }

    @Test
    fun `resetMetrics clears all counters`() {
        val cache = createCache(maxEntries = 2)
        cache.put("key1", "value1")
        cache.put("key2", "value2")
        cache.put("key3", "value3") // Eviction
        cache.get("key3") // Hit
        cache.get("missing") // Miss

        cache.resetMetrics()

        val metrics = cache.getMetrics()
        assertThat(metrics.hits).isEqualTo(0)
        assertThat(metrics.misses).isEqualTo(0)
        assertThat(metrics.evictions).isEqualTo(0)
        assertThat(metrics.expirations).isEqualTo(0)
        // Current entries/size should still reflect actual state
        assertThat(metrics.currentEntries).isEqualTo(2)
    }

    // Dynamic Limits Tests

    @Test
    fun `respects dynamic max entries limit`() {
        var dynamicMax = 5
        val cache = LruCache<String, String>(
            maxEntries = { dynamicMax },
            maxSizeBytes = { Long.MAX_VALUE },
            ttlMillis = Long.MAX_VALUE,
            sizeCalculator = { 1 }
        )

        // Fill cache with 5 entries
        repeat(5) { cache.put("key$it", "value$it") }
        assertThat(cache.getMetrics().currentEntries).isEqualTo(5)

        // Reduce limit
        dynamicMax = 3

        // Adding new entry should evict to meet new limit
        cache.put("newKey", "newValue")

        assertThat(cache.getMetrics().currentEntries).isAtMost(3)
    }

    @Test
    fun `respects dynamic max size limit`() {
        var dynamicMaxSize = 100L
        val cache = LruCache<String, String>(
            maxEntries = { 100 },
            maxSizeBytes = { dynamicMaxSize },
            ttlMillis = Long.MAX_VALUE,
            sizeCalculator = { 20 } // Each entry is 20 bytes
        )

        // Fill cache with entries totaling 80 bytes
        repeat(4) { cache.put("key$it", "value$it") }
        assertThat(cache.getMetrics().currentSizeBytes).isEqualTo(80)

        // Reduce limit
        dynamicMaxSize = 50L

        // Adding new entry should evict to meet new size limit
        cache.put("newKey", "newValue")

        assertThat(cache.getMetrics().currentSizeBytes).isAtMost(50)
    }

    // Edge Cases

    @Test
    fun `hit rate is zero when no accesses`() {
        val cache = createCache()

        val metrics = cache.getMetrics()

        assertThat(metrics.hitRate).isEqualTo(0.0)
    }

    @Test
    fun `handles concurrent access without errors`() {
        val cache = createCache(maxEntries = 100)

        // Simulate concurrent-like access
        val threads = (1..10).map { threadId ->
            Thread {
                repeat(100) { i ->
                    cache.put("thread$threadId-key$i", "value$i")
                    cache.get("thread$threadId-key$i")
                }
            }
        }

        threads.forEach { it.start() }
        threads.forEach { it.join() }

        // Should not throw and metrics should be consistent
        val metrics = cache.getMetrics()
        assertThat(metrics.hits + metrics.misses).isGreaterThan(0)
    }

    // Helper function

    private fun createCache(
        maxEntries: Int = 100,
        maxSizeBytes: Long = Long.MAX_VALUE,
        ttlMillis: Long = Long.MAX_VALUE,
        sizeCalculator: (String) -> Int = { 1 },
        timeProvider: () -> Long = { System.currentTimeMillis() }
    ): LruCache<String, String> {
        return LruCache(
            maxEntries = { maxEntries },
            maxSizeBytes = { maxSizeBytes },
            ttlMillis = ttlMillis,
            sizeCalculator = sizeCalculator,
            timeProvider = timeProvider
        )
    }
}

package com.lelloman.pezzottify.android.domain.cache

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject
import javax.inject.Singleton

interface CacheMetricsCollector {
    fun recordCacheHit(cacheType: String)
    fun recordCacheMiss(cacheType: String)
    fun recordCacheLatency(cacheType: String, latencyNanos: Long)
    fun recordDbLatency(cacheType: String, latencyNanos: Long)
    fun getReport(): CachePerformanceReport
    fun reset()
}

data class CachePerformanceReport(
    val cacheHits: Map<String, Long>,
    val cacheMisses: Map<String, Long>,
    val avgCacheLatencyNanos: Map<String, Double>,
    val avgDbLatencyNanos: Map<String, Double>,
    val hitRates: Map<String, Double>,
    val estimatedTimeSavedNanos: Long
)

@Singleton
class CacheMetricsCollectorImpl @Inject constructor() : CacheMetricsCollector {

    private val hits = ConcurrentHashMap<String, AtomicLong>()
    private val misses = ConcurrentHashMap<String, AtomicLong>()
    private val cacheLatencies = ConcurrentHashMap<String, MutableList<Long>>()
    private val dbLatencies = ConcurrentHashMap<String, MutableList<Long>>()

    override fun recordCacheHit(cacheType: String) {
        hits.computeIfAbsent(cacheType) { AtomicLong() }.incrementAndGet()
    }

    override fun recordCacheMiss(cacheType: String) {
        misses.computeIfAbsent(cacheType) { AtomicLong() }.incrementAndGet()
    }

    override fun recordCacheLatency(cacheType: String, latencyNanos: Long) {
        cacheLatencies.computeIfAbsent(cacheType) { mutableListOf() }.add(latencyNanos)
    }

    override fun recordDbLatency(cacheType: String, latencyNanos: Long) {
        dbLatencies.computeIfAbsent(cacheType) { mutableListOf() }.add(latencyNanos)
    }

    override fun getReport(): CachePerformanceReport {
        val hitCounts = hits.mapValues { it.value.get() }
        val missCounts = misses.mapValues { it.value.get() }

        val hitRates = hitCounts.keys.associateWith { key ->
            val h = hitCounts[key] ?: 0
            val m = missCounts[key] ?: 0
            if (h + m > 0) h.toDouble() / (h + m) else 0.0
        }

        val avgCacheLatency = cacheLatencies.mapValues { (_, latencies) ->
            if (latencies.isNotEmpty()) latencies.average() else 0.0
        }

        val avgDbLatency = dbLatencies.mapValues { (_, latencies) ->
            if (latencies.isNotEmpty()) latencies.average() else 0.0
        }

        // Estimate time saved: (cache hits) * (avg db latency - avg cache latency)
        val timeSaved = hitCounts.entries.sumOf { (key, count) ->
            val dbAvg = avgDbLatency[key] ?: 0.0
            val cacheAvg = avgCacheLatency[key] ?: 0.0
            (count * (dbAvg - cacheAvg)).toLong()
        }

        return CachePerformanceReport(
            cacheHits = hitCounts,
            cacheMisses = missCounts,
            avgCacheLatencyNanos = avgCacheLatency,
            avgDbLatencyNanos = avgDbLatency,
            hitRates = hitRates,
            estimatedTimeSavedNanos = timeSaved
        )
    }

    override fun reset() {
        hits.clear()
        misses.clear()
        cacheLatencies.clear()
        dbLatencies.clear()
    }
}

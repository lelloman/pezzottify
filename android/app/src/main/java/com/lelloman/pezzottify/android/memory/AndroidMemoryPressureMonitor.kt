package com.lelloman.pezzottify.android.memory

import android.app.Application
import android.content.ComponentCallbacks2
import android.content.res.Configuration
import com.lelloman.pezzottify.android.domain.memory.CacheItemType
import com.lelloman.pezzottify.android.domain.memory.MemoryInfo
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureLevel
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidMemoryPressureMonitor @Inject constructor(
    application: Application
) : MemoryPressureMonitor, ComponentCallbacks2 {

    private val _memoryInfo = MutableStateFlow(calculateMemoryInfo())
    override val memoryInfo: StateFlow<MemoryInfo> = _memoryInfo.asStateFlow()

    init {
        application.registerComponentCallbacks(this)
    }

    private fun calculateMemoryInfo(): MemoryInfo {
        val runtime = Runtime.getRuntime()
        val maxHeap = runtime.maxMemory()
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        return MemoryPressureCalculator.calculateMemoryInfo(maxHeap, usedMemory)
    }

    override fun getRecommendedCacheSizeBytes(): Long {
        return MemoryPressureCalculator.getRecommendedCacheSizeBytes(_memoryInfo.value.pressureLevel)
    }

    override fun getRecommendedMaxEntries(itemType: CacheItemType): Int {
        return MemoryPressureCalculator.getRecommendedMaxEntries(_memoryInfo.value.pressureLevel, itemType)
    }

    override fun refresh() {
        _memoryInfo.value = calculateMemoryInfo()
    }

    // ComponentCallbacks2 implementation for system memory pressure events
    override fun onTrimMemory(level: Int) {
        refresh()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        // No action needed
    }

    override fun onLowMemory() {
        _memoryInfo.value = _memoryInfo.value.copy(
            pressureLevel = MemoryPressureLevel.CRITICAL
        )
    }
}

/**
 * Extracted calculation logic for testability.
 * Contains pure functions that don't depend on Android framework.
 */
object MemoryPressureCalculator {

    // Configurable thresholds (percentage of max heap)
    private const val LOW_THRESHOLD = 0.70      // <70% used = LOW pressure
    private const val MEDIUM_THRESHOLD = 0.80   // 70-80% used = MEDIUM
    private const val HIGH_THRESHOLD = 0.90     // 80-90% used = HIGH
    // >90% = CRITICAL

    // Base cache sizes per pressure level
    private val cacheSizes = mapOf(
        MemoryPressureLevel.LOW to CacheSizeConfig(
            artistEntries = 200,
            albumEntries = 300,
            trackEntries = 500,
            totalBytes = 10 * 1024 * 1024  // 10MB
        ),
        MemoryPressureLevel.MEDIUM to CacheSizeConfig(
            artistEntries = 100,
            albumEntries = 150,
            trackEntries = 250,
            totalBytes = 5 * 1024 * 1024   // 5MB
        ),
        MemoryPressureLevel.HIGH to CacheSizeConfig(
            artistEntries = 50,
            albumEntries = 75,
            trackEntries = 100,
            totalBytes = 2 * 1024 * 1024   // 2MB
        ),
        MemoryPressureLevel.CRITICAL to CacheSizeConfig(
            artistEntries = 20,
            albumEntries = 30,
            trackEntries = 50,
            totalBytes = 512 * 1024        // 512KB
        )
    )

    fun calculateMemoryInfo(maxHeapBytes: Long, usedBytes: Long): MemoryInfo {
        val available = maxHeapBytes - usedBytes
        val usageRatio = if (maxHeapBytes > 0) {
            usedBytes.toDouble() / maxHeapBytes.toDouble()
        } else {
            1.0 // Assume critical if no heap info
        }

        val level = calculatePressureLevel(usageRatio)

        return MemoryInfo(
            availableBytes = available,
            maxHeapBytes = maxHeapBytes,
            usedBytes = usedBytes,
            pressureLevel = level
        )
    }

    fun calculatePressureLevel(usageRatio: Double): MemoryPressureLevel {
        return when {
            usageRatio < LOW_THRESHOLD -> MemoryPressureLevel.LOW
            usageRatio < MEDIUM_THRESHOLD -> MemoryPressureLevel.MEDIUM
            usageRatio < HIGH_THRESHOLD -> MemoryPressureLevel.HIGH
            else -> MemoryPressureLevel.CRITICAL
        }
    }

    fun getRecommendedCacheSizeBytes(pressureLevel: MemoryPressureLevel): Long {
        return cacheSizes[pressureLevel]?.totalBytes ?: 0
    }

    fun getRecommendedMaxEntries(pressureLevel: MemoryPressureLevel, itemType: CacheItemType): Int {
        val config = cacheSizes[pressureLevel] ?: return 0
        return when (itemType) {
            CacheItemType.ARTIST -> config.artistEntries
            CacheItemType.ALBUM -> config.albumEntries
            CacheItemType.TRACK -> config.trackEntries
        }
    }

    private data class CacheSizeConfig(
        val artistEntries: Int,
        val albumEntries: Int,
        val trackEntries: Int,
        val totalBytes: Long
    )
}

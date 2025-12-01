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

    // Configurable thresholds (percentage of max heap)
    private val lowThreshold = 0.70      // <70% used = LOW pressure
    private val mediumThreshold = 0.80   // 70-80% used = MEDIUM
    private val highThreshold = 0.90     // 80-90% used = HIGH
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

    init {
        application.registerComponentCallbacks(this)
    }

    private fun calculateMemoryInfo(): MemoryInfo {
        val runtime = Runtime.getRuntime()
        val maxHeap = runtime.maxMemory()
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        val available = maxHeap - usedMemory

        val usageRatio = usedMemory.toDouble() / maxHeap.toDouble()
        val level = when {
            usageRatio < lowThreshold -> MemoryPressureLevel.LOW
            usageRatio < mediumThreshold -> MemoryPressureLevel.MEDIUM
            usageRatio < highThreshold -> MemoryPressureLevel.HIGH
            else -> MemoryPressureLevel.CRITICAL
        }

        return MemoryInfo(
            availableBytes = available,
            maxHeapBytes = maxHeap,
            usedBytes = usedMemory,
            pressureLevel = level
        )
    }

    override fun getRecommendedCacheSizeBytes(): Long {
        return cacheSizes[_memoryInfo.value.pressureLevel]?.totalBytes ?: 0
    }

    override fun getRecommendedMaxEntries(itemType: CacheItemType): Int {
        val config = cacheSizes[_memoryInfo.value.pressureLevel] ?: return 0
        return when (itemType) {
            CacheItemType.ARTIST -> config.artistEntries
            CacheItemType.ALBUM -> config.albumEntries
            CacheItemType.TRACK -> config.trackEntries
        }
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

    private data class CacheSizeConfig(
        val artistEntries: Int,
        val albumEntries: Int,
        val trackEntries: Int,
        val totalBytes: Long
    )
}

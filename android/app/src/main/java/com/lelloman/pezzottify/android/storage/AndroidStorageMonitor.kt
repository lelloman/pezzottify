package com.lelloman.pezzottify.android.storage

import android.app.Application
import android.os.StatFs
import com.lelloman.pezzottify.android.domain.storage.StorageInfo
import com.lelloman.pezzottify.android.domain.storage.StorageMonitor
import com.lelloman.pezzottify.android.domain.storage.StoragePressureLevel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidStorageMonitor @Inject constructor(
    private val application: Application
) : StorageMonitor {

    private val _storageInfo = MutableStateFlow(calculateStorageInfo())
    override val storageInfo: StateFlow<StorageInfo> = _storageInfo.asStateFlow()

    private fun calculateStorageInfo(): StorageInfo {
        val filesDir = application.filesDir
        val stat = StatFs(filesDir.absolutePath)

        val totalBytes = stat.totalBytes
        val availableBytes = stat.availableBytes
        val usedBytes = totalBytes - stat.freeBytes

        return StoragePressureCalculator.calculateStorageInfo(
            totalBytes = totalBytes,
            availableBytes = availableBytes,
            usedBytes = usedBytes
        )
    }

    override fun canAllocate(sizeBytes: Long, keepBuffer: Boolean): Boolean {
        val available = _storageInfo.value.availableBytes
        return if (keepBuffer) {
            // Keep 100MB buffer for system operations
            val buffer = StoragePressureCalculator.SAFETY_BUFFER_BYTES
            available - sizeBytes > buffer
        } else {
            available >= sizeBytes
        }
    }

    override fun getRecommendedMaxCacheBytes(): Long {
        return StoragePressureCalculator.getRecommendedMaxCacheBytes(
            _storageInfo.value.pressureLevel,
            _storageInfo.value.availableBytes
        )
    }

    override fun refresh() {
        _storageInfo.value = calculateStorageInfo()
    }
}

/**
 * Extracted calculation logic for testability.
 * Contains pure functions that don't depend on Android framework.
 */
object StoragePressureCalculator {

    // Safety buffer to keep for system operations (100MB)
    const val SAFETY_BUFFER_BYTES = 100 * 1024 * 1024L

    // Storage pressure thresholds (percentage of available storage)
    private const val LOW_THRESHOLD = 0.20      // <20% available = CRITICAL
    private const val MEDIUM_THRESHOLD = 0.10   // 10-20% available = HIGH
    private const val HIGH_THRESHOLD = 0.05     // 5-10% available = MEDIUM
    // >20% available = LOW

    // Maximum cache sizes based on pressure level and available storage
    private val cachePercentages = mapOf(
        StoragePressureLevel.LOW to 0.15,      // Use up to 15% of available for cache
        StoragePressureLevel.MEDIUM to 0.10,   // Use up to 10% of available for cache
        StoragePressureLevel.HIGH to 0.05,     // Use up to 5% of available for cache
        StoragePressureLevel.CRITICAL to 0.01  // Use up to 1% of available for cache
    )

    fun calculateStorageInfo(
        totalBytes: Long,
        availableBytes: Long,
        usedBytes: Long
    ): StorageInfo {
        val availablePercentage = if (totalBytes > 0) {
            availableBytes.toDouble() / totalBytes.toDouble()
        } else {
            0.0
        }

        val pressureLevel = calculatePressureLevel(availablePercentage)

        return StorageInfo(
            totalBytes = totalBytes,
            availableBytes = availableBytes,
            usedBytes = usedBytes,
            pressureLevel = pressureLevel
        )
    }

    fun calculatePressureLevel(availablePercentage: Double): StoragePressureLevel {
        return when {
            availablePercentage >= LOW_THRESHOLD -> StoragePressureLevel.LOW
            availablePercentage >= MEDIUM_THRESHOLD -> StoragePressureLevel.MEDIUM
            availablePercentage >= HIGH_THRESHOLD -> StoragePressureLevel.HIGH
            else -> StoragePressureLevel.CRITICAL
        }
    }

    fun getRecommendedMaxCacheBytes(
        pressureLevel: StoragePressureLevel,
        availableBytes: Long
    ): Long {
        val percentage = cachePercentages[pressureLevel] ?: 0.01
        return (availableBytes * percentage).toLong()
    }
}

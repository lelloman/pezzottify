package com.lelloman.pezzottify.android.memory

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.memory.CacheItemType
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureLevel
import org.junit.Test

class MemoryPressureCalculatorTest {

    // Pressure Level Calculation Tests

    @Test
    fun `calculatePressureLevel returns LOW when usage below 70 percent`() {
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.0))
            .isEqualTo(MemoryPressureLevel.LOW)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.50))
            .isEqualTo(MemoryPressureLevel.LOW)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.69))
            .isEqualTo(MemoryPressureLevel.LOW)
    }

    @Test
    fun `calculatePressureLevel returns MEDIUM when usage between 70 and 80 percent`() {
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.70))
            .isEqualTo(MemoryPressureLevel.MEDIUM)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.75))
            .isEqualTo(MemoryPressureLevel.MEDIUM)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.79))
            .isEqualTo(MemoryPressureLevel.MEDIUM)
    }

    @Test
    fun `calculatePressureLevel returns HIGH when usage between 80 and 90 percent`() {
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.80))
            .isEqualTo(MemoryPressureLevel.HIGH)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.85))
            .isEqualTo(MemoryPressureLevel.HIGH)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.89))
            .isEqualTo(MemoryPressureLevel.HIGH)
    }

    @Test
    fun `calculatePressureLevel returns CRITICAL when usage above 90 percent`() {
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.90))
            .isEqualTo(MemoryPressureLevel.CRITICAL)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(0.95))
            .isEqualTo(MemoryPressureLevel.CRITICAL)
        assertThat(MemoryPressureCalculator.calculatePressureLevel(1.0))
            .isEqualTo(MemoryPressureLevel.CRITICAL)
    }

    // Memory Info Calculation Tests

    @Test
    fun `calculateMemoryInfo correctly calculates available bytes`() {
        val maxHeap = 100_000_000L  // 100MB
        val used = 30_000_000L      // 30MB

        val info = MemoryPressureCalculator.calculateMemoryInfo(maxHeap, used)

        assertThat(info.availableBytes).isEqualTo(70_000_000L)
        assertThat(info.maxHeapBytes).isEqualTo(maxHeap)
        assertThat(info.usedBytes).isEqualTo(used)
    }

    @Test
    fun `calculateMemoryInfo returns LOW pressure for low usage`() {
        val maxHeap = 100_000_000L  // 100MB
        val used = 50_000_000L      // 50MB (50%)

        val info = MemoryPressureCalculator.calculateMemoryInfo(maxHeap, used)

        assertThat(info.pressureLevel).isEqualTo(MemoryPressureLevel.LOW)
    }

    @Test
    fun `calculateMemoryInfo returns MEDIUM pressure for moderate usage`() {
        val maxHeap = 100_000_000L  // 100MB
        val used = 75_000_000L      // 75MB (75%)

        val info = MemoryPressureCalculator.calculateMemoryInfo(maxHeap, used)

        assertThat(info.pressureLevel).isEqualTo(MemoryPressureLevel.MEDIUM)
    }

    @Test
    fun `calculateMemoryInfo returns HIGH pressure for high usage`() {
        val maxHeap = 100_000_000L  // 100MB
        val used = 85_000_000L      // 85MB (85%)

        val info = MemoryPressureCalculator.calculateMemoryInfo(maxHeap, used)

        assertThat(info.pressureLevel).isEqualTo(MemoryPressureLevel.HIGH)
    }

    @Test
    fun `calculateMemoryInfo returns CRITICAL pressure for very high usage`() {
        val maxHeap = 100_000_000L  // 100MB
        val used = 95_000_000L      // 95MB (95%)

        val info = MemoryPressureCalculator.calculateMemoryInfo(maxHeap, used)

        assertThat(info.pressureLevel).isEqualTo(MemoryPressureLevel.CRITICAL)
    }

    @Test
    fun `calculateMemoryInfo handles zero max heap as CRITICAL`() {
        val info = MemoryPressureCalculator.calculateMemoryInfo(0L, 0L)

        assertThat(info.pressureLevel).isEqualTo(MemoryPressureLevel.CRITICAL)
    }

    // Cache Size Recommendation Tests

    @Test
    fun `getRecommendedCacheSizeBytes returns 10MB for LOW pressure`() {
        val size = MemoryPressureCalculator.getRecommendedCacheSizeBytes(MemoryPressureLevel.LOW)

        assertThat(size).isEqualTo(10 * 1024 * 1024)
    }

    @Test
    fun `getRecommendedCacheSizeBytes returns 5MB for MEDIUM pressure`() {
        val size = MemoryPressureCalculator.getRecommendedCacheSizeBytes(MemoryPressureLevel.MEDIUM)

        assertThat(size).isEqualTo(5 * 1024 * 1024)
    }

    @Test
    fun `getRecommendedCacheSizeBytes returns 2MB for HIGH pressure`() {
        val size = MemoryPressureCalculator.getRecommendedCacheSizeBytes(MemoryPressureLevel.HIGH)

        assertThat(size).isEqualTo(2 * 1024 * 1024)
    }

    @Test
    fun `getRecommendedCacheSizeBytes returns 512KB for CRITICAL pressure`() {
        val size = MemoryPressureCalculator.getRecommendedCacheSizeBytes(MemoryPressureLevel.CRITICAL)

        assertThat(size).isEqualTo(512 * 1024)
    }

    // Max Entries Recommendation Tests - LOW pressure

    @Test
    fun `getRecommendedMaxEntries returns 200 artists for LOW pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.LOW,
            CacheItemType.ARTIST
        )

        assertThat(entries).isEqualTo(200)
    }

    @Test
    fun `getRecommendedMaxEntries returns 300 albums for LOW pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.LOW,
            CacheItemType.ALBUM
        )

        assertThat(entries).isEqualTo(300)
    }

    @Test
    fun `getRecommendedMaxEntries returns 500 tracks for LOW pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.LOW,
            CacheItemType.TRACK
        )

        assertThat(entries).isEqualTo(500)
    }

    // Max Entries Recommendation Tests - MEDIUM pressure

    @Test
    fun `getRecommendedMaxEntries returns 100 artists for MEDIUM pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.MEDIUM,
            CacheItemType.ARTIST
        )

        assertThat(entries).isEqualTo(100)
    }

    @Test
    fun `getRecommendedMaxEntries returns 150 albums for MEDIUM pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.MEDIUM,
            CacheItemType.ALBUM
        )

        assertThat(entries).isEqualTo(150)
    }

    @Test
    fun `getRecommendedMaxEntries returns 250 tracks for MEDIUM pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.MEDIUM,
            CacheItemType.TRACK
        )

        assertThat(entries).isEqualTo(250)
    }

    // Max Entries Recommendation Tests - HIGH pressure

    @Test
    fun `getRecommendedMaxEntries returns 50 artists for HIGH pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.HIGH,
            CacheItemType.ARTIST
        )

        assertThat(entries).isEqualTo(50)
    }

    @Test
    fun `getRecommendedMaxEntries returns 75 albums for HIGH pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.HIGH,
            CacheItemType.ALBUM
        )

        assertThat(entries).isEqualTo(75)
    }

    @Test
    fun `getRecommendedMaxEntries returns 100 tracks for HIGH pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.HIGH,
            CacheItemType.TRACK
        )

        assertThat(entries).isEqualTo(100)
    }

    // Max Entries Recommendation Tests - CRITICAL pressure

    @Test
    fun `getRecommendedMaxEntries returns 20 artists for CRITICAL pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.CRITICAL,
            CacheItemType.ARTIST
        )

        assertThat(entries).isEqualTo(20)
    }

    @Test
    fun `getRecommendedMaxEntries returns 30 albums for CRITICAL pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.CRITICAL,
            CacheItemType.ALBUM
        )

        assertThat(entries).isEqualTo(30)
    }

    @Test
    fun `getRecommendedMaxEntries returns 50 tracks for CRITICAL pressure`() {
        val entries = MemoryPressureCalculator.getRecommendedMaxEntries(
            MemoryPressureLevel.CRITICAL,
            CacheItemType.TRACK
        )

        assertThat(entries).isEqualTo(50)
    }

    // Verification that cache sizes decrease with pressure

    @Test
    fun `cache sizes decrease as pressure increases`() {
        val levels = listOf(
            MemoryPressureLevel.LOW,
            MemoryPressureLevel.MEDIUM,
            MemoryPressureLevel.HIGH,
            MemoryPressureLevel.CRITICAL
        )

        val cacheSizes = levels.map { MemoryPressureCalculator.getRecommendedCacheSizeBytes(it) }
        val artistEntries = levels.map { MemoryPressureCalculator.getRecommendedMaxEntries(it, CacheItemType.ARTIST) }
        val albumEntries = levels.map { MemoryPressureCalculator.getRecommendedMaxEntries(it, CacheItemType.ALBUM) }
        val trackEntries = levels.map { MemoryPressureCalculator.getRecommendedMaxEntries(it, CacheItemType.TRACK) }

        // Each subsequent value should be smaller
        for (i in 0 until levels.size - 1) {
            assertThat(cacheSizes[i]).isGreaterThan(cacheSizes[i + 1])
            assertThat(artistEntries[i]).isGreaterThan(artistEntries[i + 1])
            assertThat(albumEntries[i]).isGreaterThan(albumEntries[i + 1])
            assertThat(trackEntries[i]).isGreaterThan(trackEntries[i + 1])
        }
    }
}

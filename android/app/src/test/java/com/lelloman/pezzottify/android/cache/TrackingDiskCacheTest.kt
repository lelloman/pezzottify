package com.lelloman.pezzottify.android.cache

import com.google.common.truth.Truth.assertThat
import okio.FileSystem
import okio.buffer
import org.junit.Test
import java.util.UUID

class TrackingDiskCacheTest {

    @Test
    fun `rebuilds entry index after cache recreation`() {
        val directory = FileSystem.SYSTEM_TEMPORARY_DIRECTORY
            .resolve("pezzottify-cache-${UUID.randomUUID()}")
        try {
            val firstCache = TrackingDiskCache.create(directory, 1024 * 1024)
            firstCache.openEditor("cached-image")!!.run {
                FileSystem.SYSTEM.sink(data).buffer().use { it.writeUtf8("image") }
                FileSystem.SYSTEM.sink(metadata).buffer().use { it.writeUtf8("metadata") }
                commit()
            }
            firstCache.shutdown()

            val recreatedCache = TrackingDiskCache.create(directory, 1024 * 1024)

            assertThat(recreatedCache.getEntryCount()).isEqualTo(1)
            assertThat(recreatedCache.trimOldestPercent(0.5f)).isGreaterThan(0L)
            assertThat(recreatedCache.openSnapshot("cached-image")).isNull()
            recreatedCache.shutdown()
        } finally {
            FileSystem.SYSTEM.deleteRecursively(directory, mustExist = false)
        }
    }
}

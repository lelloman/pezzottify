package com.lelloman.pezzottify.android.cache

import coil3.disk.DiskCache
import okio.FileSystem
import okio.Path
import java.util.concurrent.ConcurrentHashMap

/**
 * A [DiskCache] wrapper that tracks cache keys and their insertion times,
 * enabling age-based trimming operations.
 *
 * This wrapper delegates all operations to an underlying [DiskCache] while
 * maintaining an index of keys and timestamps. This allows us to implement
 * [trimOldestPercent] without relying on implementation details of the
 * underlying cache.
 */
class TrackingDiskCache(
    private val delegate: DiskCache,
) : DiskCache {

    /**
     * Index of cache keys to their insertion timestamp.
     * Using ConcurrentHashMap for thread safety.
     */
    private val keyIndex = ConcurrentHashMap<String, Long>()

    override val directory: Path
        get() = delegate.directory

    override val fileSystem: FileSystem
        get() = delegate.fileSystem

    override val maxSize: Long
        get() = delegate.maxSize

    override val size: Long
        get() = delegate.size

    override fun openSnapshot(key: String): DiskCache.Snapshot? {
        return delegate.openSnapshot(key)
    }

    override fun openEditor(key: String): DiskCache.Editor? {
        val editor = delegate.openEditor(key)
        if (editor != null) {
            // Wrap the editor to track when the entry is committed
            return TrackingEditor(key, editor)
        }
        return null
    }

    override fun remove(key: String): Boolean {
        val removed = delegate.remove(key)
        if (removed) {
            keyIndex.remove(key)
        }
        return removed
    }

    override fun clear() {
        delegate.clear()
        keyIndex.clear()
    }

    override fun shutdown() {
        delegate.shutdown()
        // Don't clear keyIndex on shutdown - it's just closing resources
    }

    /**
     * Returns the number of tracked entries.
     */
    fun getEntryCount(): Int = keyIndex.size

    /**
     * Trims the oldest entries by percentage.
     *
     * @param percent The percentage of entries to remove (0.0 to 1.0)
     * @return The number of bytes freed
     */
    fun trimOldestPercent(percent: Float): Long {
        require(percent in 0f..1f) { "Percent must be between 0 and 1" }

        if (keyIndex.isEmpty()) {
            return 0L
        }

        val sizeBefore = size

        // Sort entries by timestamp (oldest first) and take the oldest N percent
        val sortedEntries = keyIndex.entries
            .sortedBy { it.value }

        val countToRemove = (sortedEntries.size * percent).toInt().coerceAtLeast(1)
        val entriesToRemove = sortedEntries.take(countToRemove)

        entriesToRemove.forEach { (key, _) ->
            remove(key)
        }

        return sizeBefore - size
    }

    /**
     * Wrapper for [DiskCache.Editor] that records the key when the entry is committed.
     */
    private inner class TrackingEditor(
        private val key: String,
        private val delegate: DiskCache.Editor,
    ) : DiskCache.Editor {

        override val data: Path
            get() = delegate.data

        override val metadata: Path
            get() = delegate.metadata

        override fun commit() {
            delegate.commit()
            // Record the key with current timestamp after successful commit
            keyIndex[key] = System.currentTimeMillis()
        }

        override fun commitAndOpenSnapshot(): DiskCache.Snapshot? {
            val snapshot = delegate.commitAndOpenSnapshot()
            if (snapshot != null) {
                // Record the key with current timestamp after successful commit
                keyIndex[key] = System.currentTimeMillis()
            }
            return snapshot
        }

        override fun abort() {
            delegate.abort()
            // Don't record anything on abort
        }
    }

    companion object {
        /**
         * Creates a [TrackingDiskCache] wrapping a new [DiskCache] built with the given configuration.
         */
        fun create(
            directory: Path,
            maxSizeBytes: Long,
            fileSystem: FileSystem = FileSystem.SYSTEM,
        ): TrackingDiskCache {
            val delegate = DiskCache.Builder()
                .directory(directory)
                .maxSizeBytes(maxSizeBytes)
                .fileSystem(fileSystem)
                .build()
            return TrackingDiskCache(delegate)
        }
    }
}

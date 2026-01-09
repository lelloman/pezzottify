package com.lelloman.pezzottify.android.domain.skeleton

/**
 * Interface for storing and querying catalog skeleton data.
 *
 * The skeleton is an on-demand cache of catalog entity relationships.
 * It stores artist-album and album-track IDs for efficient queries
 * without fetching full entity data from the server.
 *
 * This is NOT a sync mechanism - data is cached on-demand as needed.
 */
interface SkeletonStore {

    /**
     * Get all album IDs for a given artist from cache.
     * Returns empty list if artist not found in cache.
     */
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    /**
     * Observe album IDs for a given artist as a Flow.
     * This is reactive and will emit updates when cache data changes.
     */
    fun observeAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>>

    /**
     * Get all track IDs for a given album from cache.
     * Returns empty list if album not found in cache.
     */
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    /**
     * Insert artist-album relationships into cache.
     * Used when fetching discography for an artist.
     */
    suspend fun insertAlbumArtists(albumArtists: List<AlbumArtistRelationship>)

    /**
     * Delete all cached album IDs for a given artist.
     * Used to invalidate cache when needed.
     */
    suspend fun deleteAlbumsForArtist(artistId: String): Result<Unit>

    /**
     * Clear all skeleton cache data.
     * Used for migration or full cache invalidation.
     */
    suspend fun clear(): Result<Unit>
}

/**
 * Artist-album relationship for caching.
 * @param orderIndex The position in the server's sorted discography (by availability, then popularity/date)
 */
data class AlbumArtistRelationship(
    val artistId: String,
    val albumId: String,
    val orderIndex: Int = 0
)

/**
 * Counts of skeleton entities.
 */
data class SkeletonCounts(
    val artists: Int,
    val albums: Int,
    val tracks: Int
)

/**
 * Full skeleton data from server.
 */
data class FullSkeleton(
    val version: Long,
    val checksum: String,
    val artists: List<String>,
    val albums: List<SkeletonAlbumData>,
    val tracks: List<SkeletonTrackData>
)

/**
 * Album data in skeleton (ID + artist IDs).
 */
data class SkeletonAlbumData(
    val id: String,
    val artistIds: List<String>
)

/**
 * Track data in skeleton (ID + album ID).
 */
data class SkeletonTrackData(
    val id: String,
    val albumId: String
)

/**
 * Delta changes from server.
 */
data class SkeletonDelta(
    val fromVersion: Long,
    val toVersion: Long,
    val checksum: String,
    val changes: List<SkeletonChange>
)

/**
 * A single change in the skeleton.
 */
sealed class SkeletonChange {
    data class ArtistAdded(val artistId: String) : SkeletonChange()
    data class ArtistRemoved(val artistId: String) : SkeletonChange()
    data class AlbumAdded(val albumId: String, val artistIds: List<String>) : SkeletonChange()
    data class AlbumRemoved(val albumId: String) : SkeletonChange()
    data class TrackAdded(val trackId: String, val albumId: String) : SkeletonChange()
    data class TrackRemoved(val trackId: String) : SkeletonChange()
}

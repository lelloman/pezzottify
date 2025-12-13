package com.lelloman.pezzottify.android.domain.skeleton

/**
 * Interface for storing and querying catalog skeleton data.
 *
 * The skeleton contains just IDs and relationships for all catalog entities,
 * enabling efficient sync and discography queries without fetching full data.
 */
interface SkeletonStore {

    /**
     * Get the current skeleton version.
     * Returns null if skeleton has never been synced.
     */
    suspend fun getVersion(): Long?

    /**
     * Get the current skeleton checksum.
     * Returns null if skeleton has never been synced.
     */
    suspend fun getChecksum(): String?

    /**
     * Get all album IDs for a given artist.
     * Returns empty list if artist not found in skeleton.
     */
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    /**
     * Get all track IDs for a given album.
     * Returns empty list if album not found in skeleton.
     */
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    /**
     * Get counts of all skeleton entities.
     */
    suspend fun getCounts(): SkeletonCounts

    /**
     * Replace all skeleton data with a full sync result.
     * This should be called inside a transaction.
     */
    suspend fun replaceAll(fullSkeleton: FullSkeleton): Result<Unit>

    /**
     * Apply delta changes to the skeleton.
     * Returns failure if the delta cannot be applied (e.g., version mismatch).
     */
    suspend fun applyDelta(delta: SkeletonDelta): Result<Unit>

    /**
     * Clear all skeleton data.
     */
    suspend fun clear(): Result<Unit>
}

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

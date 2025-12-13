package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Provider for artist discography data using the local skeleton store.
 *
 * This replaces the need to fetch discography from the server API,
 * as the skeleton already contains all album IDs for each artist.
 */
@Singleton
class DiscographyProvider @Inject constructor(
    private val skeletonStore: SkeletonStore
) {
    /**
     * Get album IDs for an artist from the local skeleton.
     * Always returns current data (no cache staleness).
     *
     * @param artistId The artist ID to look up
     * @return List of album IDs for the artist, empty if artist not in skeleton
     */
    suspend fun getAlbumIdsForArtist(artistId: String): List<String> {
        return skeletonStore.getAlbumIdsForArtist(artistId)
    }

    /**
     * Get track IDs for an album from the local skeleton.
     *
     * @param albumId The album ID to look up
     * @return List of track IDs for the album, empty if album not in skeleton
     */
    suspend fun getTrackIdsForAlbum(albumId: String): List<String> {
        return skeletonStore.getTrackIdsForAlbum(albumId)
    }
}

package com.lelloman.pezzottify.android.domain.statics

import kotlinx.coroutines.flow.Flow

interface StaticsStore {

    fun getArtist(artistId: String): Flow<Artist?>

    fun getTrack(trackId: String): Flow<Track?>

    fun getAlbum(albumId: String): Flow<Album?>

    suspend fun storeArtist(artist: Artist): Result<Unit>

    suspend fun storeTrack(track: Track): Result<Unit>

    suspend fun storeAlbum(album: Album): Result<Unit>

    suspend fun deleteAll(): Result<Unit>

    /**
     * Returns the total number of entries (artists + albums + tracks) in the database.
     */
    suspend fun countEntries(): Int

    /**
     * Trims the database by removing the oldest entries by cachedAt timestamp.
     * @param percent The percentage of entries to remove (0.0 to 1.0)
     * @return The total number of entries removed
     */
    suspend fun trimOldestPercent(percent: Float): Int

    /**
     * Returns the database file size in bytes.
     */
    suspend fun getDatabaseSizeBytes(): Long

    /**
     * Compacts the database by running VACUUM.
     * This reclaims space from deleted rows.
     */
    suspend fun vacuum()
}
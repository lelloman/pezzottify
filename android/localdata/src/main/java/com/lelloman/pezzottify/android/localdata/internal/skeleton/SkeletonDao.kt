package com.lelloman.pezzottify.android.localdata.internal.skeleton

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbum
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonMeta
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonTrack

@Dao
internal interface SkeletonDao {

    // =========================================================================
    // Queries
    // =========================================================================

    @Query("SELECT ${SkeletonMeta.COLUMN_VALUE} FROM ${SkeletonMeta.TABLE_NAME} WHERE ${SkeletonMeta.COLUMN_KEY} = '${SkeletonMeta.KEY_VERSION}'")
    suspend fun getVersion(): String?

    @Query("SELECT ${SkeletonMeta.COLUMN_VALUE} FROM ${SkeletonMeta.TABLE_NAME} WHERE ${SkeletonMeta.COLUMN_KEY} = '${SkeletonMeta.KEY_CHECKSUM}'")
    suspend fun getChecksum(): String?

    @Query("SELECT ${SkeletonAlbumArtist.COLUMN_ALBUM_ID} FROM ${SkeletonAlbumArtist.TABLE_NAME} WHERE ${SkeletonAlbumArtist.COLUMN_ARTIST_ID} = :artistId")
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    @Query("SELECT ${SkeletonAlbumArtist.COLUMN_ALBUM_ID} FROM ${SkeletonAlbumArtist.TABLE_NAME} WHERE ${SkeletonAlbumArtist.COLUMN_ARTIST_ID} = :artistId")
    fun observeAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>>

    @Query("SELECT ${SkeletonTrack.COLUMN_ID} FROM ${SkeletonTrack.TABLE_NAME} WHERE ${SkeletonTrack.COLUMN_ALBUM_ID} = :albumId")
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    @Query("SELECT COUNT(*) FROM ${SkeletonArtist.TABLE_NAME}")
    suspend fun getArtistCount(): Int

    @Query("SELECT COUNT(*) FROM ${SkeletonAlbum.TABLE_NAME}")
    suspend fun getAlbumCount(): Int

    @Query("SELECT COUNT(*) FROM ${SkeletonTrack.TABLE_NAME}")
    suspend fun getTrackCount(): Int

    // =========================================================================
    // Insert Operations
    // =========================================================================

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtists(artists: List<SkeletonArtist>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbums(albums: List<SkeletonAlbum>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbumArtists(albumArtists: List<SkeletonAlbumArtist>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTracks(tracks: List<SkeletonTrack>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun setMeta(meta: SkeletonMeta)

    // Single entity inserts (for delta sync)
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: SkeletonArtist)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: SkeletonAlbum)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTrack(track: SkeletonTrack)

    // =========================================================================
    // Delete Operations
    // =========================================================================

    @Query("DELETE FROM ${SkeletonTrack.TABLE_NAME}")
    suspend fun deleteAllTracks()

    @Query("DELETE FROM ${SkeletonAlbumArtist.TABLE_NAME}")
    suspend fun deleteAllAlbumArtists()

    @Query("DELETE FROM ${SkeletonAlbum.TABLE_NAME}")
    suspend fun deleteAllAlbums()

    @Query("DELETE FROM ${SkeletonArtist.TABLE_NAME}")
    suspend fun deleteAllArtists()

    @Query("DELETE FROM ${SkeletonArtist.TABLE_NAME} WHERE ${SkeletonArtist.COLUMN_ID} = :artistId")
    suspend fun deleteArtist(artistId: String)

    @Query("DELETE FROM ${SkeletonAlbum.TABLE_NAME} WHERE ${SkeletonAlbum.COLUMN_ID} = :albumId")
    suspend fun deleteAlbum(albumId: String)

    @Query("DELETE FROM ${SkeletonTrack.TABLE_NAME} WHERE ${SkeletonTrack.COLUMN_ID} = :trackId")
    suspend fun deleteTrack(trackId: String)

    // =========================================================================
    // Transaction Operations
    // =========================================================================

    /**
     * Replace all skeleton data with new full sync data.
     * This is wrapped in a transaction to ensure atomicity.
     */
    @Transaction
    suspend fun replaceAll(
        artists: List<SkeletonArtist>,
        albums: List<SkeletonAlbum>,
        albumArtists: List<SkeletonAlbumArtist>,
        tracks: List<SkeletonTrack>,
        version: String,
        checksum: String
    ) {
        // Delete in reverse order of dependencies
        deleteAllTracks()
        deleteAllAlbumArtists()
        deleteAllAlbums()
        deleteAllArtists()

        // Insert in order of dependencies
        insertArtists(artists)
        insertAlbums(albums)
        insertAlbumArtists(albumArtists)
        insertTracks(tracks)

        // Update metadata
        setMeta(SkeletonMeta(SkeletonMeta.KEY_VERSION, version))
        setMeta(SkeletonMeta(SkeletonMeta.KEY_CHECKSUM, checksum))
    }

    /**
     * Apply a delta update, wrapped in a transaction.
     */
    @Transaction
    suspend fun applyDelta(
        addedArtists: List<SkeletonArtist>,
        removedArtistIds: List<String>,
        addedAlbums: List<SkeletonAlbum>,
        addedAlbumArtists: List<SkeletonAlbumArtist>,
        removedAlbumIds: List<String>,
        addedTracks: List<SkeletonTrack>,
        removedTrackIds: List<String>,
        newVersion: String,
        newChecksum: String
    ) {
        // Apply removals first (cascade will handle related data)
        removedTrackIds.forEach { deleteTrack(it) }
        removedAlbumIds.forEach { deleteAlbum(it) }
        removedArtistIds.forEach { deleteArtist(it) }

        // Apply additions
        if (addedArtists.isNotEmpty()) insertArtists(addedArtists)
        if (addedAlbums.isNotEmpty()) insertAlbums(addedAlbums)
        if (addedAlbumArtists.isNotEmpty()) insertAlbumArtists(addedAlbumArtists)
        if (addedTracks.isNotEmpty()) insertTracks(addedTracks)

        // Update metadata
        setMeta(SkeletonMeta(SkeletonMeta.KEY_VERSION, newVersion))
        setMeta(SkeletonMeta(SkeletonMeta.KEY_CHECKSUM, newChecksum))
    }
}

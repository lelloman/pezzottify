package com.lelloman.pezzottify.android.localdata.internal.skeleton

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbum
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonMeta
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonTrack

@Dao
internal interface SkeletonDao {

    // =========================================================================
    // Cache Queries
    // =========================================================================

    @Query("SELECT ${SkeletonAlbumArtist.COLUMN_ALBUM_ID} FROM ${SkeletonAlbumArtist.TABLE_NAME} WHERE ${SkeletonAlbumArtist.COLUMN_ARTIST_ID} = :artistId")
    suspend fun getAlbumIdsForArtist(artistId: String): List<String>

    @Query("SELECT ${SkeletonAlbumArtist.COLUMN_ALBUM_ID} FROM ${SkeletonAlbumArtist.TABLE_NAME} WHERE ${SkeletonAlbumArtist.COLUMN_ARTIST_ID} = :artistId")
    fun observeAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>>

    @Query("SELECT ${SkeletonTrack.COLUMN_ID} FROM ${SkeletonTrack.TABLE_NAME} WHERE ${SkeletonTrack.COLUMN_ALBUM_ID} = :albumId")
    suspend fun getTrackIdsForAlbum(albumId: String): List<String>

    // =========================================================================
    // Insert Operations
    // =========================================================================

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbumArtists(albumArtists: List<SkeletonAlbumArtist>)

    // =========================================================================
    // Delete Operations
    // =========================================================================

    @Query("DELETE FROM ${SkeletonAlbumArtist.TABLE_NAME} WHERE ${SkeletonAlbumArtist.COLUMN_ARTIST_ID} = :artistId")
    suspend fun deleteAlbumsForArtist(artistId: String)

    @Query("DELETE FROM ${SkeletonTrack.TABLE_NAME}")
    suspend fun deleteAllTracks()

    @Query("DELETE FROM ${SkeletonAlbumArtist.TABLE_NAME}")
    suspend fun deleteAllAlbumArtists()

    @Query("DELETE FROM ${SkeletonAlbum.TABLE_NAME}")
    suspend fun deleteAllAlbums()

    @Query("DELETE FROM ${SkeletonArtist.TABLE_NAME}")
    suspend fun deleteAllArtists()

    @Query("DELETE FROM ${SkeletonMeta.TABLE_NAME}")
    suspend fun deleteAllMeta()
}

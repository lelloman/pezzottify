package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Album
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Track
import kotlinx.coroutines.flow.Flow

@Dao
internal interface StaticsDao {

    // Get
    @Query("SELECT * FROM ${Artist.TABLE_NAME} WHERE ${Artist.COLUMN_ID} = :id")
    fun getArtist(id: String): Flow<Artist?>

    @Query("SELECT * FROM ${Track.TABLE_NAME} WHERE ${Track.COLUMN_ID} = :id")
    fun getTrack(id: String): Flow<Track?>

    @Query("SELECT * FROM ${Album.TABLE_NAME} WHERE ${Album.COLUMN_ID} = :id")
    fun getAlbum(id: String): Flow<Album?>

    // Insert
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: Artist): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTrack(track: Track): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: Album): Long

    // Delete
    @Query("DELETE FROM ${Artist.TABLE_NAME} WHERE ${Artist.COLUMN_ID} = :artistId")
    suspend fun deleteArtist(artistId: String): Int

    @Query("DELETE FROM ${Track.TABLE_NAME} WHERE ${Track.COLUMN_ID} = :trackId")
    suspend fun deleteTrack(trackId: String): Int

    @Query("DELETE FROM ${Album.TABLE_NAME} WHERE ${Album.COLUMN_ID} = :albumId")
    suspend fun deleteAlbum(albumId: String): Int

    // Count
    @Query("SELECT COUNT(*) FROM ${Artist.TABLE_NAME}")
    suspend fun countArtists(): Int

    @Query("SELECT COUNT(*) FROM ${Album.TABLE_NAME}")
    suspend fun countAlbums(): Int

    @Query("SELECT COUNT(*) FROM ${Track.TABLE_NAME}")
    suspend fun countTracks(): Int

    // Delete oldest by cachedAt (for trimming)
    @Query(
        """
        DELETE FROM ${Artist.TABLE_NAME}
        WHERE ${Artist.COLUMN_ID} IN (
            SELECT ${Artist.COLUMN_ID} FROM ${Artist.TABLE_NAME}
            ORDER BY ${Artist.COLUMN_CACHED_AT} ASC
            LIMIT :limit
        )
        """
    )
    suspend fun deleteOldestArtists(limit: Int): Int

    @Query(
        """
        DELETE FROM ${Album.TABLE_NAME}
        WHERE ${Album.COLUMN_ID} IN (
            SELECT ${Album.COLUMN_ID} FROM ${Album.TABLE_NAME}
            ORDER BY ${Album.COLUMN_CACHED_AT} ASC
            LIMIT :limit
        )
        """
    )
    suspend fun deleteOldestAlbums(limit: Int): Int

    @Query(
        """
        DELETE FROM ${Track.TABLE_NAME}
        WHERE ${Track.COLUMN_ID} IN (
            SELECT ${Track.COLUMN_ID} FROM ${Track.TABLE_NAME}
            ORDER BY ${Track.COLUMN_CACHED_AT} ASC
            LIMIT :limit
        )
        """
    )
    suspend fun deleteOldestTracks(limit: Int): Int
}

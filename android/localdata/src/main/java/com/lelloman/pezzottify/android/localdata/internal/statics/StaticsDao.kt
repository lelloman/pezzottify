package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Album
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.internal.statics.model.ArtistDiscography
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

    @Query("SELECT * FROM ${ArtistDiscography.TABLE_NAME} WHERE ${ArtistDiscography.COLUMN_ARTIST_ID} = :artistId LIMIT 1")
    fun getArtistDiscography(artistId: String): Flow<ArtistDiscography?>

    // Insert
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: Artist): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTrack(track: Track): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: Album): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtistDiscography(artistDiscography: ArtistDiscography): Long

    // Delete
    @Query("DELETE FROM ${Artist.TABLE_NAME} WHERE ${Artist.COLUMN_ID} = :artistId")
    suspend fun deleteArtist(artistId: String): Int

    @Query("DELETE FROM ${Track.TABLE_NAME} WHERE ${Track.COLUMN_ID} = :trackId")
    suspend fun deleteTrack(trackId: String): Int

    @Query("DELETE FROM ${Album.TABLE_NAME} WHERE ${Album.COLUMN_ID} = :albumId")
    suspend fun deleteAlbum(albumId: String): Int

    @Query("DELETE FROM ${ArtistDiscography.TABLE_NAME} WHERE ${ArtistDiscography.COLUMN_ARTIST_ID} = :artistId")
    suspend fun deleteArtistDiscography(artistId: String): Int
}
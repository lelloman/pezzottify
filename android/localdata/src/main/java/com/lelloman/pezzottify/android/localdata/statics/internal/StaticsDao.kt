package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.statics.model.Album
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.statics.model.Track
import kotlinx.coroutines.flow.Flow

@Dao
interface StaticsDao {

    @Query("SELECT * FROM ${Artist.TABLE_NAME} WHERE ${Artist.COLUMN_ID} = :id")
    fun getArtist(id: String): Flow<Artist?>

    @Query("SELECT * FROM ${Track.TABLE_NAME} WHERE ${Track.COLUMN_ID} = :id")
    fun getTrack(id: String): Flow<Track?>

    @Query("SELECT * FROM ${Album.TABLE_NAME} WHERE ${Album.COLUMN_ID} = :id")
    fun getAlbum(id: String): Flow<Album?>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertArtist(artist: Artist): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTrack(track: Track): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAlbum(album: Album): Long

}
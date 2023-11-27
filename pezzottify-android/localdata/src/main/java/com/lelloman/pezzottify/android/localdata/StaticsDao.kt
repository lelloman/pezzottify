package com.lelloman.pezzottify.android.localdata

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Transaction
import com.lelloman.pezzottify.android.localdata.model.Album
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.Image
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import kotlinx.coroutines.flow.Flow

@Dao
interface StaticsDao {

    @Query("SELECT * FROM ${IndividualArtist.TABLE_NAME}")
    fun getIndividuals(): Flow<List<IndividualArtist>>

    @Query("SELECT * FROM ${BandArtist.TABLE_NAME}")
    fun getBands(): Flow<List<BandArtist>>

    @Query("SELECT * FROM ${Album.TABLE_NAME}")
    fun getAlbums(): Flow<List<Album>>

    @Query("DELETE FROM ${IndividualArtist.TABLE_NAME}")
    fun deleteIndividuals()

    @Query("DELETE FROM ${BandArtist.TABLE_NAME}")
    fun deleteBands()

    @Query("DELETE FROM ${Image.TABLE_NAME}")
    fun deleteImages()

    @Query("DELETE FROM ${AudioTrack.TABLE_NAME}")
    fun deleteAudioTracks()

    @Query("DELETE FROM ${Album.TABLE_NAME}")
    fun deleteAlbums()

    @Insert
    fun insertIndividuals(individuals: List<IndividualArtist>)

    @Insert
    fun insertBands(bands: List<BandArtist>)

    @Insert
    fun insertImages(images: List<Image>)

    @Insert
    fun insertAudioTracks(tracks: List<AudioTrack>)

    @Insert
    fun insertAlbums(albums: List<Album>)

    @Query("SELECT * FROM ${Album.TABLE_NAME} WHERE id=:id")
    suspend fun getAlbum(id: String): Album?

    @Transaction
    fun replaceStatics(
        albums: List<Album>,
        individuals: List<IndividualArtist>,
        bands: List<BandArtist>,
        images: List<Image>,
        audioTracks: List<AudioTrack>,
    ) {
        deleteIndividuals()
        deleteBands()
        deleteImages()
        deleteAudioTracks()
        deleteAlbums()

        insertIndividuals(individuals)
        insertBands(bands)
        insertImages(images)
        insertAudioTracks(audioTracks)
        insertAlbums(albums)
    }
}
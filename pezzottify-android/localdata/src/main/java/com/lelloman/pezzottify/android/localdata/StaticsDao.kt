package com.lelloman.pezzottify.android.localdata

import androidx.room.Dao
import androidx.room.Query
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import kotlinx.coroutines.flow.Flow

@Dao
interface StaticsDao {

    @Query("SELECT * FROM ${IndividualArtist.TABLE_NAME}")
    fun getIndividuals(): Flow<List<IndividualArtist>>

    @Query("SELECT * FROM ${BandArtist.TABLE_NAME}")
    fun getBands(): Flow<List<BandArtist>>
}
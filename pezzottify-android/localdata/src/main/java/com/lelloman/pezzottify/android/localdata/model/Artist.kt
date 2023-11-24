package com.lelloman.pezzottify.android.localdata.model

import androidx.room.Entity
import androidx.room.PrimaryKey

interface Artist {
    val id: String
    val displayName: String
    val imageId: String?
}

@Entity(tableName = IndividualArtist.TABLE_NAME)
data class IndividualArtist(
    @PrimaryKey
    override val id: String = "",
    override val displayName: String,
    override val imageId: String? = null,
    val firstName: String? = null,
    val lastName: String? = null,
) : Artist {
    companion object {
        const val TABLE_NAME = "individual_artist"
    }
}

@Entity(tableName = BandArtist.TABLE_NAME)
data class BandArtist(
    @PrimaryKey
    override val id: String = "",
    override val displayName: String,
    override val imageId: String? = null,
    val membersIds: List<String>,
) : Artist {
    companion object {
        const val TABLE_NAME = "band_artist"
    }
}
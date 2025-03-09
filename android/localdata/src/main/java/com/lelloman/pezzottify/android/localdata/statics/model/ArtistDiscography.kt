package com.lelloman.pezzottify.android.localdata.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.localdata.statics.model.ArtistDiscography.Companion.COLUMN_ARTIST_ID

@Entity(
    tableName = ArtistDiscography.TABLE_NAME,
    foreignKeys = [
        ForeignKey(
            entity = Artist::class,
            parentColumns = [Artist.COLUMN_ID],
            childColumns = [COLUMN_ARTIST_ID],
            onDelete = ForeignKey.CASCADE,
            onUpdate = ForeignKey.CASCADE,
        )
    ]
)
data class ArtistDiscography(

    @PrimaryKey
    @ColumnInfo(name = COLUMN_ARTIST_ID)
    override val artistId: String,

    override val albumsIds: List<String>,

    override val featuresIds: List<String>,

    val created: Long,
) : com.lelloman.pezzottify.android.domain.statics.ArtistDiscography {

    val id: String get() = getId(artistId)

    companion object {
        const val TABLE_NAME = "artist_discography"

        const val COLUMN_ARTIST_ID = "artist_id"

        fun getId(artistId: String) = "${artistId}_discography"
    }
}

fun com.lelloman.pezzottify.android.domain.statics.ArtistDiscography.quack(): ArtistDiscography =
    ArtistDiscography(
        artistId = artistId,
        albumsIds = albumsIds,
        featuresIds = featuresIds,
        created = System.currentTimeMillis(),
    )
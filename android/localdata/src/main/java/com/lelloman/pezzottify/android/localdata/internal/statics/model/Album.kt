package com.lelloman.pezzottify.android.localdata.internal.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.statics.AlbumAvailability
import com.lelloman.pezzottify.android.domain.statics.AlbumEnrichment
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus
import kotlinx.serialization.Serializable

@Serializable
data class Disc(
    override val tracksIds: List<String>,
) : com.lelloman.pezzottify.android.domain.statics.Disc

@Entity
internal data class Album(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val date: Int,

    override val displayImageId: String?,

    override val artistsIds: List<String>,

    override val discs: List<Disc>,

    @ColumnInfo(name = COLUMN_AVAILABILITY, defaultValue = "missing")
    override val availability: AlbumAvailability = AlbumAvailability.Missing,

    @ColumnInfo(name = COLUMN_ENRICHMENT_STATUS)
    override val enrichmentStatus: EntityEnrichmentStatus? = null,

    @ColumnInfo(name = COLUMN_ENRICHMENT)
    override val enrichment: AlbumEnrichment? = null,

    @ColumnInfo(name = COLUMN_CACHED_AT, defaultValue = "0")
    val cachedAt: Long = System.currentTimeMillis(),

    ) : com.lelloman.pezzottify.android.domain.statics.Album {

    companion object {
        const val TABLE_NAME = "album"

        const val COLUMN_ID = "id"
        const val COLUMN_AVAILABILITY = "availability"
        const val COLUMN_CACHED_AT = "cached_at"
        const val COLUMN_ENRICHMENT_STATUS = "enrichment_status"
        const val COLUMN_ENRICHMENT = "enrichment"
    }
}

internal fun com.lelloman.pezzottify.android.domain.statics.Album.quack(): Album = Album(
    id = id,
    name = name,
    date = date,
    displayImageId = displayImageId,
    artistsIds = artistsIds,
    discs = discs.map { Disc(tracksIds = it.tracksIds) },
    availability = availability,
    enrichmentStatus = enrichmentStatus,
    enrichment = enrichment,
    cachedAt = System.currentTimeMillis(),
)
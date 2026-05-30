package com.lelloman.pezzottify.android.localdata.internal.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.statics.ArtistEnrichment
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus

@Entity(tableName = Artist.TABLE_NAME)
internal data class Artist(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val displayImageId: String?,

    override val related: List<String>,

    @ColumnInfo(name = COLUMN_ENRICHMENT_STATUS)
    override val enrichmentStatus: EntityEnrichmentStatus? = null,

    @ColumnInfo(name = COLUMN_ENRICHMENT)
    override val enrichment: ArtistEnrichment? = null,

    @ColumnInfo(name = COLUMN_CACHED_AT, defaultValue = "0")
    val cachedAt: Long = System.currentTimeMillis(),
) : com.lelloman.pezzottify.android.domain.statics.Artist {

    companion object {
        const val TABLE_NAME = "artist"

        const val COLUMN_ID = "id"
        const val COLUMN_CACHED_AT = "cached_at"
        const val COLUMN_ENRICHMENT_STATUS = "enrichment_status"
        const val COLUMN_ENRICHMENT = "enrichment"
    }
}

internal fun com.lelloman.pezzottify.android.domain.statics.Artist.quack(): Artist = Artist(
    id = id,
    name = name,
    displayImageId = displayImageId,
    related = related,
    enrichmentStatus = enrichmentStatus,
    enrichment = enrichment,
    cachedAt = System.currentTimeMillis(),
)
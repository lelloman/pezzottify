package com.lelloman.pezzottify.android.localdata.internal.statics.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus
import com.lelloman.pezzottify.android.domain.statics.TrackEnrichment

@Entity(tableName = Track.TABLE_NAME)
internal data class Track(

    @ColumnInfo(name = COLUMN_ID)
    @PrimaryKey
    override val id: String,

    override val name: String,

    override val albumId: String,

    override val artistsIds: List<String>,

    override val durationSeconds: Int,

    @ColumnInfo(name = COLUMN_AVAILABILITY, defaultValue = "available")
    val availabilityString: String = "available",

    @ColumnInfo(name = COLUMN_ENRICHMENT_STATUS)
    override val enrichmentStatus: EntityEnrichmentStatus? = null,

    @ColumnInfo(name = COLUMN_ENRICHMENT)
    override val enrichment: TrackEnrichment? = null,

    @ColumnInfo(name = COLUMN_CACHED_AT, defaultValue = "0")
    val cachedAt: Long = System.currentTimeMillis(),
) : com.lelloman.pezzottify.android.domain.statics.Track {

    override val availability: TrackAvailability
        get() = TrackAvailability.fromServerString(availabilityString)

    companion object {
        const val TABLE_NAME = "Track"

        const val COLUMN_ID = "id"
        const val COLUMN_AVAILABILITY = "availability"
        const val COLUMN_CACHED_AT = "cached_at"
        const val COLUMN_ENRICHMENT_STATUS = "enrichment_status"
        const val COLUMN_ENRICHMENT = "enrichment"
    }
}

internal fun com.lelloman.pezzottify.android.domain.statics.Track.quack(): Track = Track(
    id = id,
    name = name,
    albumId = albumId,
    artistsIds = artistsIds,
    durationSeconds = durationSeconds,
    availabilityString = availability.name.lowercase(),
    enrichmentStatus = enrichmentStatus,
    enrichment = enrichment,
    cachedAt = System.currentTimeMillis(),
)
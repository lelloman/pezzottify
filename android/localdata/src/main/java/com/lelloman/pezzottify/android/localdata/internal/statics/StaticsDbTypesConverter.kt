package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.TypeConverter
import com.lelloman.pezzottify.android.domain.statics.AlbumAvailability
import com.lelloman.pezzottify.android.domain.statics.AlbumEnrichment
import com.lelloman.pezzottify.android.domain.statics.ArtistEnrichment
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus
import com.lelloman.pezzottify.android.domain.statics.TrackEnrichment
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Disc
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

internal object StaticsDbTypesConverter {

    private val json = Json {
        ignoreUnknownKeys = true
    }

    @TypeConverter
    fun fromListOfStringsToString(values: List<String>): String = json.encodeToString(values)

    @TypeConverter
    fun fromStringToListOfStrings(value: String): List<String> = json.decodeFromString(value)

    @TypeConverter
    fun fromDiscsListToString(values: List<Disc>): String = json.encodeToString(values)

    @TypeConverter
    fun fromStringToDiscsList(value: String): List<Disc> = json.decodeFromString(value)

    @TypeConverter
    fun fromAlbumAvailabilityToString(value: AlbumAvailability): String = when (value) {
        AlbumAvailability.Complete -> "complete"
        AlbumAvailability.Partial -> "partial"
        AlbumAvailability.Missing -> "missing"
    }

    @TypeConverter
    fun fromStringToAlbumAvailability(value: String): AlbumAvailability =
        AlbumAvailability.fromServerString(value)

    @TypeConverter
    fun fromEntityEnrichmentStatusToString(value: EntityEnrichmentStatus?): String? =
        value?.let { json.encodeToString(EntityEnrichmentStatus.serializer(), it) }

    @TypeConverter
    fun fromStringToEntityEnrichmentStatus(value: String?): EntityEnrichmentStatus? =
        value?.let { json.decodeFromString(EntityEnrichmentStatus.serializer(), it) }

    @TypeConverter
    fun fromArtistEnrichmentToString(value: ArtistEnrichment?): String? =
        value?.let { json.encodeToString(ArtistEnrichment.serializer(), it) }

    @TypeConverter
    fun fromStringToArtistEnrichment(value: String?): ArtistEnrichment? =
        value?.let { json.decodeFromString(ArtistEnrichment.serializer(), it) }

    @TypeConverter
    fun fromAlbumEnrichmentToString(value: AlbumEnrichment?): String? =
        value?.let { json.encodeToString(AlbumEnrichment.serializer(), it) }

    @TypeConverter
    fun fromStringToAlbumEnrichment(value: String?): AlbumEnrichment? =
        value?.let { json.decodeFromString(AlbumEnrichment.serializer(), it) }

    @TypeConverter
    fun fromTrackEnrichmentToString(value: TrackEnrichment?): String? =
        value?.let { json.encodeToString(TrackEnrichment.serializer(), it) }

    @TypeConverter
    fun fromStringToTrackEnrichment(value: String?): TrackEnrichment? =
        value?.let { json.decodeFromString(TrackEnrichment.serializer(), it) }
}

package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus
import com.lelloman.pezzottify.android.domain.statics.TrackEnrichment
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class ArtistRole {
    MainArtist,
    FeaturedArtist,
    Remixer,
    Composer,
    Conductor,
    Orchestra,
}

/**
 * Artist with their role on a track.
 */
@Serializable
data class TrackArtist(
    val artist: ArtistData,
    val role: ArtistRole,
)

/**
 * Server's ResolvedTrack response - nested structure with track, album, and artists.
 */
@Serializable
data class TrackResponse(
    val track: TrackData,
    val album: AlbumData,
    val artists: List<TrackArtist>,
    @SerialName("enrichment_status")
    val enrichmentStatus: EntityEnrichmentStatus? = null,
    val enrichment: TrackEnrichment? = null,
    @SerialName("work_resolution") val workResolution: com.lelloman.pezzottify.android.domain.statics.WorkResolution? = null,
    @SerialName("work_enrichment_status") val workEnrichmentStatus: EntityEnrichmentStatus? = null,
    @SerialName("recording_work") val recordingWork: com.lelloman.pezzottify.android.domain.statics.RecordingWork? = null,
    @SerialName("relationship_scope") val relationshipScope: String? = null,
)

fun TrackResponse.toDomain() = object : Track {
    override val workResolution get() = this@toDomain.workResolution
    override val workEnrichmentStatus get() = this@toDomain.workEnrichmentStatus
    override val id: String
        get() = this@toDomain.track.id
    override val name: String
        get() = this@toDomain.track.name
    override val albumId: String
        get() = this@toDomain.track.albumId
    override val artistsIds: List<String>
        get() = this@toDomain.artists.map { it.artist.id }
    override val durationSeconds: Int
        get() = (this@toDomain.track.durationMs / 1000).toInt()
    override val availability: TrackAvailability
        get() = this@toDomain.track.availability
    override val enrichmentStatus: EntityEnrichmentStatus?
        get() = this@toDomain.enrichmentStatus
    override val enrichment: TrackEnrichment?
        get() = this@toDomain.enrichment
}
package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Track
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
    Actor,
    Unknown,
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
)

fun TrackResponse.toDomain() = object : Track {
    override val id: String
        get() = this@toDomain.track.id
    override val name: String
        get() = this@toDomain.track.name
    override val albumId: String
        get() = this@toDomain.track.albumId
    override val artistsIds: List<String>
        get() = this@toDomain.artists.map { it.artist.id }
    override val durationSeconds: Int
        get() = this@toDomain.track.durationSecs ?: 0
}
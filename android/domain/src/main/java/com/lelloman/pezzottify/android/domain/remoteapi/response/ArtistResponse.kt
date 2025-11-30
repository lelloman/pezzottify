package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Artist
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Inner artist data from server's ResolvedArtist response.
 */
@Serializable
data class ArtistData(
    val id: String,
    val name: String,
    val genres: List<String>,
    @SerialName("activity_periods")
    val activityPeriods: List<ActivityPeriod>,
)

/**
 * Server's ResolvedArtist response - nested structure with artist, display_image, and related artists.
 */
@Serializable
data class ArtistResponse(
    val artist: ArtistData,
    @SerialName("display_image")
    val displayImage: Image?,
    @SerialName("related_artists")
    val relatedArtists: List<ArtistData>,
)

fun ArtistResponse.toDomain() = object : Artist {
    override val id: String
        get() = this@toDomain.artist.id
    override val name: String
        get() = this@toDomain.artist.name
    override val displayImageId: String?
        get() = this@toDomain.displayImage?.id
    override val related: List<String>
        get() = this@toDomain.relatedArtists.map { it.id }
}
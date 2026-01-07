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
 * Server's ResolvedArtist response - nested structure with artist and related artists.
 * Images are fetched by artist ID via /v1/content/image/{id}
 */
@Serializable
data class ArtistResponse(
    val artist: ArtistData,
    @SerialName("related_artists")
    val relatedArtists: List<ArtistData>,
)

fun ArtistResponse.toDomain() = object : Artist {
    override val id: String
        get() = this@toDomain.artist.id
    override val name: String
        get() = this@toDomain.artist.name
    override val displayImageId: String?
        get() = this@toDomain.artist.id // Images are fetched by artist ID
    override val related: List<String>
        get() = this@toDomain.relatedArtists.map { it.id }
}
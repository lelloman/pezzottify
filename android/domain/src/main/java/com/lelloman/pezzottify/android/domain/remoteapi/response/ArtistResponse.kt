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
 * Server's ResolvedArtist response - nested structure with artist, images, and related artists.
 */
@Serializable
data class ArtistResponse(
    val artist: ArtistData,
    val images: List<Image>,
    @SerialName("related_artists")
    val relatedArtists: List<ArtistData>,
)

fun ArtistResponse.toDomain() = object : Artist {
    override val id: String
        get() = this@toDomain.artist.id
    override val name: String
        get() = this@toDomain.artist.name
    override val portraits: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = this@toDomain.images.map { it.toDomain() }
    override val portraitGroup: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = emptyList() // Server doesn't differentiate image types currently
    override val related: List<String>
        get() = this@toDomain.relatedArtists.map { it.id }
}

private fun Image.toDomain() = com.lelloman.pezzottify.android.domain.statics.Image(
    id = id,
    size = when (size) {
        ImageSize.Small -> com.lelloman.pezzottify.android.domain.statics.ImageSize.SMALL
        ImageSize.Default -> com.lelloman.pezzottify.android.domain.statics.ImageSize.DEFAULT
        ImageSize.Large -> com.lelloman.pezzottify.android.domain.statics.ImageSize.LARGE
        ImageSize.XLarge -> com.lelloman.pezzottify.android.domain.statics.ImageSize.XLARGE
    }
)
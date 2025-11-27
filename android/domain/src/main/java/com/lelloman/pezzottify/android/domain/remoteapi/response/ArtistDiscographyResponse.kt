package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.ArtistDiscography
import kotlinx.serialization.Serializable

@Serializable
data class ArtistDiscographyResponse(
    val albums: List<String>,
    val features: List<String>,
)

fun ArtistDiscographyResponse.toDomain(artistId: String) = object : ArtistDiscography {
    override val artistId: String
        get() = artistId
    override val albumsIds: List<String>
        get() = this@toDomain.albums
    override val featuresIds: List<String>
        get() = this@toDomain.features
}
package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.ArtistDiscography
import kotlinx.serialization.Serializable

/**
 * Server's ArtistDiscography response - contains Album objects, not just IDs.
 */
@Serializable
data class ArtistDiscographyResponse(
    val albums: List<AlbumData>,
    val features: List<AlbumData>,
)

fun ArtistDiscographyResponse.toDomain(artistId: String) = object : ArtistDiscography {
    override val artistId: String
        get() = artistId
    override val albumsIds: List<String>
        get() = this@toDomain.albums.map { it.id }
    override val featuresIds: List<String>
        get() = this@toDomain.features.map { it.id }
}
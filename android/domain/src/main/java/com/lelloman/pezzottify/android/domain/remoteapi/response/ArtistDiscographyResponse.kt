package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.ArtistDiscography
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

private fun convertReleaseDateToInt(releaseDate: String?): Int {
    return releaseDate?.let { date ->
        when {
            date.matches(Regex("^\\d{4}$")) -> {
                date.toInt() * 10000
            }
            date.matches(Regex("^\\d{4}-\\d{2}$")) -> {
                val parts = date.split("-")
                parts[0].toInt() * 10000 + parts[1].toInt() * 100
            }
            date.matches(Regex("^\\d{4}-\\d{2}-\\d{2}$")) -> {
                val parts = date.split("-")
                parts[0].toInt() * 10000 + parts[1].toInt() * 100 + parts[2].toInt()
            }
            else -> 0
        }
    } ?: 0
}

/**
 * Full Album model as returned in artist discography.
 * Matches server's Album struct from catalog_store/models.rs
 */
@Serializable
data class DiscographyAlbum(
    val id: String,
    val name: String,
    @SerialName("album_type")
    val albumType: AlbumType,
    val label: String? = null,
    @SerialName("release_date")
    val releaseDate: String? = null,
    @SerialName("release_date_precision")
    val releaseDatePrecision: String? = null,
    @SerialName("external_id_upc")
    val externalIdUpc: String? = null,
)

/**
 * Server's ArtistDiscography response.
 * GET /v1/content/artist/{id}/discography
 */
@Serializable
data class ArtistDiscographyResponse(
    val albums: List<DiscographyAlbum>,
    val total: Int = 0,
    @SerialName("has_more")
    val hasMore: Boolean = false,
    @SerialName("offset")
    val offset: Int? = null,
    @SerialName("limit")
    val limit: Int? = null,
)

fun ArtistDiscographyResponse.toDomain(artistId: String) = object : ArtistDiscography {
    override val artistId: String
        get() = artistId
    override val albumsIds: List<String>
        get() = this@toDomain.albums.map { it.id }
    override val featuresIds: List<String>
        get() = emptyList()
}
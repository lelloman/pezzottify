package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Album
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

private fun convertReleaseDateToInt(releaseDate: String): Int {
    return when {
        releaseDate.matches(Regex("^\\d{4}$")) -> {
            releaseDate.toInt() * 10000
        }
        releaseDate.matches(Regex("^\\d{4}-\\d{2}$")) -> {
            val parts = releaseDate.split("-")
            parts[0].toInt() * 10000 + parts[1].toInt() * 100
        }
        releaseDate.matches(Regex("^\\d{4}-\\d{2}-\\d{2}$")) -> {
            val parts = releaseDate.split("-")
            parts[0].toInt() * 10000 + parts[1].toInt() * 100 + parts[2].toInt()
        }
        else -> 0
    }
}

@Serializable
enum class AlbumType {
    @SerialName("album")
    Album,
    @SerialName("single")
    Single,
    @SerialName("compilation")
    Compilation,
}

/**
 * Track data embedded in album disc.
 */
@Serializable
data class TrackData(
    val id: String,
    val name: String,
    @SerialName("album_id")
    val albumId: String,
    @SerialName("disc_number")
    val discNumber: Int,
    @SerialName("track_number")
    val trackNumber: Int,
    @SerialName("duration_ms")
    val durationMs: Long,
)

@Serializable
data class Disc(
    val number: Int,
    val tracks: List<TrackData>,
)

/**
 * Inner album data from server's ResolvedAlbum response.
 */
@Serializable
data class AlbumData(
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
    val popularity: Int = 0,
)

/**
 * Server's ResolvedAlbum response - nested structure with album, artists, and discs.
 * Images are fetched by album ID via /v1/content/image/{id}
 */
@Serializable
data class AlbumResponse(
    val album: AlbumData,
    val artists: List<ArtistData>,
    val discs: List<Disc>,
)

fun AlbumResponse.toDomain() = object : Album {
    override val id: String
        get() = this@toDomain.album.id
    override val name: String
        get() = this@toDomain.album.name
    override val date: Int
        get() = convertReleaseDateToInt(this@toDomain.album.releaseDate ?: "")
    override val artistsIds: List<String>
        get() = this@toDomain.artists.map { it.id }
    override val displayImageId: String?
        get() = this@toDomain.album.id
    override val discs: List<com.lelloman.pezzottify.android.domain.statics.Disc>
        get() = this@toDomain.discs.map {
            object : com.lelloman.pezzottify.android.domain.statics.Disc {
                override val tracksIds: List<String> = it.tracks.map { track -> track.id }
            }
        }
}
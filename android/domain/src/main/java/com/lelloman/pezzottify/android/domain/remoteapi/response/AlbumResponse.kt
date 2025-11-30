package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Album
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class AlbumType {
    Album,
    Single,
    Ep,
    Compilation,
    Audiobook,
    Podcast,
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
    @SerialName("duration_secs")
    val durationSecs: Int?,
    @SerialName("is_explicit")
    val isExplicit: Boolean,
    @SerialName("audio_uri")
    val audioUri: String,
    val format: String,
    val tags: List<String>,
    @SerialName("has_lyrics")
    val hasLyrics: Boolean,
    val languages: List<String>,
    @SerialName("original_title")
    val originalTitle: String?,
    @SerialName("version_title")
    val versionTitle: String?,
)

@Serializable
data class Disc(
    val number: Int,
    val name: String?,
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
    val label: String?,
    @SerialName("release_date")
    val releaseDate: Long?,
    val genres: List<String>,
    @SerialName("original_title")
    val originalTitle: String?,
    @SerialName("version_title")
    val versionTitle: String?,
)

/**
 * Server's ResolvedAlbum response - nested structure with album, artists, discs, and images.
 */
@Serializable
data class AlbumResponse(
    val album: AlbumData,
    val artists: List<ArtistData>,
    val discs: List<Disc>,
    val images: List<Image>,
)

fun AlbumResponse.toDomain() = object : Album {
    override val id: String
        get() = this@toDomain.album.id
    override val name: String
        get() = this@toDomain.album.name
    override val date: Long
        get() = this@toDomain.album.releaseDate ?: 0L
    override val artistsIds: List<String>
        get() = this@toDomain.artists.map { it.id }
    override val coverGroup: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = emptyList() // Server doesn't differentiate image types currently
    override val covers: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = this@toDomain.images.map { it.toDomain() }
    override val genre: List<String>
        get() = this@toDomain.album.genres
    override val related: List<String>
        get() = emptyList() // Server doesn't provide related albums currently
    override val discs: List<com.lelloman.pezzottify.android.domain.statics.Disc>
        get() = this@toDomain.discs.map {
            object : com.lelloman.pezzottify.android.domain.statics.Disc {
                override val name: String? = it.name
                override val tracksIds: List<String> = it.tracks.map { track -> track.id }
            }
        }
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
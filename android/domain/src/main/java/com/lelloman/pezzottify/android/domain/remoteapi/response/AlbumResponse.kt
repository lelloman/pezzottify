package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Album
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class AlbumType {
    ALBUM,
    SINGLE,
    COMPILATION,
    EP,
    AUDIOBOOK,
    PODCAST,
}

@Serializable
data class Disc(
    val number: Int,
    val name: String,
    val tracks: List<String>,
)

@Serializable
data class AlbumResponse(
    val id: String,

    val name: String,

    @SerialName("album_type")
    val albumType: AlbumType,

    @SerialName("artists_ids")
    val artistsIds: List<String>,

    val label: String,

    val date: Long,

    @SerialName("genres")
    val genre: List<String>,

    val covers: List<Image>,

    val discs: List<Disc>,

    val related: List<String>,

    @SerialName("cover_group")
    val coverGroup: List<Image>,

    @SerialName("original_title")
    val originalTitle: String,

    @SerialName("version_title")
    val versionTitle: String,

    val typeStr: String,
)

fun AlbumResponse.toDomain() = object : Album {
    override val id: String
        get() = this@toDomain.id
    override val name: String
        get() = this@toDomain.name
    override val artistsIds: List<String>
        get() = this@toDomain.artistsIds
    override val coverGroup: List<String> = this@toDomain.coverGroup.map { it.id }
    override val covers: List<String> = this@toDomain.covers.map { it.id }
    override val genre: List<String> = this@toDomain.genre
    override val related: List<String> = this@toDomain.related
    override val discs: List<com.lelloman.pezzottify.android.domain.statics.Disc> =
        this@toDomain.discs.map {
            object : com.lelloman.pezzottify.android.domain.statics.Disc {
                override val name: String = it.name
                override val tracksIds: List<String> = it.tracks
            }
        }
}
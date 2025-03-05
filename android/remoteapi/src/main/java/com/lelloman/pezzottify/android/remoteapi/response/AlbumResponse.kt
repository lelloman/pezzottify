package com.lelloman.pezzottify.android.remoteapi.response

import kotlinx.serialization.SerialName

enum class AlbumType {
    ALBUM,
    SINGLE,
    COMPILATION,
    EP,
    AUDIOBOOK,
    PODCAST,
}

data class Disc(
    val number: Int,
    val name: String,
    val tracks: List<String>,
)

data class AlbumResponse(
    val id: String,

    val name: String,

    @SerialName("album_type")
    val albumType: AlbumType,

    @SerialName("artists_ids")
    val artistsIds: List<String>,

    val label: String,

    val date: Long,

    val genres: List<String>,

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
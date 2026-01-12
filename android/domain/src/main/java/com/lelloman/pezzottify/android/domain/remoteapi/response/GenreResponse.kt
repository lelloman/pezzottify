package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class GenreResponse(
    val name: String,
    @SerialName("track_count")
    val trackCount: Int,
)

typealias GenresResponse = List<GenreResponse>

@Serializable
data class GenreTrackResponse(
    val id: String,
    val name: String,
    @SerialName("duration_ms")
    val durationMs: Long,
    @SerialName("album_id")
    val albumId: String,
    @SerialName("album_name")
    val albumName: String,
    @SerialName("artist_names")
    val artistNames: List<String>,
    @SerialName("image_id")
    val imageId: String? = null,
    val availability: String,
)

@Serializable
data class GenreTracksResponse(
    @SerialName("track_ids")
    val trackIds: List<String>,
    val total: Int,
    @SerialName("has_more")
    val hasMore: Boolean,
)

package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * A popular album with listening statistics.
 */
@Serializable
data class PopularAlbum(
    val id: String,
    val name: String,
    @SerialName("artist_names")
    val artistNames: List<String>,
    @SerialName("play_count")
    val playCount: Long,
)

/**
 * A popular artist with listening statistics.
 */
@Serializable
data class PopularArtist(
    val id: String,
    val name: String,
    @SerialName("play_count")
    val playCount: Long,
)

/**
 * Response from /v1/content/popular endpoint.
 * Contains popular albums and artists based on listening data.
 */
@Serializable
data class PopularContentResponse(
    val albums: List<PopularAlbum>,
    val artists: List<PopularArtist>,
)

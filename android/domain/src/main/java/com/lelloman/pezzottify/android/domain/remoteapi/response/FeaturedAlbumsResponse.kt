package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class FeaturedAlbumResponse(
    val id: String,
    val name: String,
    @SerialName("artist_names")
    val artistNames: List<String>,
    @SerialName("play_count")
    val playCount: Long,
)

@Serializable
data class FeaturedAlbumsResponse(
    @SerialName("week_key")
    val weekKey: String,
    @SerialName("generated_at")
    val generatedAt: Long,
    @SerialName("hero_index")
    val heroIndex: Int,
    val albums: List<FeaturedAlbumResponse>,
)

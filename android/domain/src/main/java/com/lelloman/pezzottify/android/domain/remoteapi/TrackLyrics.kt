package com.lelloman.pezzottify.android.domain.remoteapi

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class TrackLyrics(
    @SerialName("track_id") val trackId: String,
    val status: String,
    val provider: String = "lrclib",
    @SerialName("plain_lyrics") val plainLyrics: String? = null,
    @SerialName("synced_lyrics") val syncedLyrics: String? = null,
)

package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class AddTracksToPlaylistRequest(
    @SerialName("tracks_ids")
    val tracksIds: List<String>,
)

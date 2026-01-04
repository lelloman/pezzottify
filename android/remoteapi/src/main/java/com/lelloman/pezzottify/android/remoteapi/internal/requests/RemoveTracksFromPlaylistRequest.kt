package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class RemoveTracksFromPlaylistRequest(
    @SerialName("tracks_positions")
    val tracksPositions: List<Int>,
)

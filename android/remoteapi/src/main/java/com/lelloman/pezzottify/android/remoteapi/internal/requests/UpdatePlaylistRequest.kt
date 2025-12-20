package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class UpdatePlaylistRequest(
    val name: String?,
    @SerialName("track_ids")
    val trackIds: List<String>?,
)

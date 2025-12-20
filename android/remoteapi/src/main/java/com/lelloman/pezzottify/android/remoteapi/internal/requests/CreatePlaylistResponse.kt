package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.Serializable

@Serializable
internal data class CreatePlaylistResponse(
    val id: String,
)

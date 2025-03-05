package com.lelloman.pezzottify.android.remoteapi.response

import kotlinx.serialization.Serializable

@Serializable
data class ArtistDiscographyResponse(
    val albums: List<String>,
    val features: List<String>,
)
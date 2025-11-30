package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.Serializable

@Serializable
enum class ImageSize {
    Small,
    Default,
    Large,
    XLarge,
}

@Serializable
data class Image(
    val id: String,
    val uri: String,
    val size: ImageSize,
    val width: Int,
    val height: Int,
)
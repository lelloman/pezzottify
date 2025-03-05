package com.lelloman.pezzottify.android.remoteapi.response

import kotlinx.serialization.Serializable


enum class ImageSize {
    DEFAULT,
    SMALL,
    MEDIUM,
    LARGE,
    XLARGE,
}

@Serializable
data class Image(
    val id: String,
    val size: ImageSize,
    val width: Int,
    val height: Int,
)
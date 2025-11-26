package com.lelloman.pezzottify.android.domain.statics

import kotlinx.serialization.Serializable

/**
 * Represents an image in the catalog with size information.
 */
@Serializable
data class Image(
    val id: String,
    val size: ImageSize,
)

/**
 * Image size categories used for prioritization and selection.
 * Matches the server's image size classifications.
 */
@Serializable
enum class ImageSize {
    DEFAULT,
    SMALL,
    MEDIUM,
    LARGE,
    XLARGE,
}

package com.lelloman.pezzottify.android.ui.model

data class LikedContent(
    val contentId: String,
    val contentType: ContentType,
    val isLiked: Boolean,
) {
    enum class ContentType {
        Album,
        Artist,
        Track,
    }
}

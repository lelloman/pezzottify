package com.lelloman.pezzottify.android.domain.usercontent

interface LikedContent {
    val contentId: String
    val contentType: ContentType
    val isLiked: Boolean
    val modifiedAt: Long
    val syncStatus: SyncStatus

    enum class ContentType {
        Album,
        Artist,
        Track,
    }
}

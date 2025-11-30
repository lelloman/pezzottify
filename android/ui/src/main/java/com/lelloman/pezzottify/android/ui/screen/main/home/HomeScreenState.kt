package com.lelloman.pezzottify.android.ui.screen.main.home

import com.lelloman.pezzottify.android.ui.content.Content
import kotlinx.coroutines.flow.Flow

data class HomeScreenState(
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
) {
    data class RecentlyViewedContent(
        val contentId: String,
        val contentType: ViewedContentType,
    )
}

data class ResolvedRecentlyViewedContent(
    val contentId: String,
    val contentName: String,
    val contentImageUrl: String?,
    val contentType: ViewedContentType,
)

enum class ViewedContentType {
    Artist,
    Album,
    Track,
    Playlist,
}
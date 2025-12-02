package com.lelloman.pezzottify.android.ui.screen.main.home

import com.lelloman.pezzottify.android.ui.content.Content
import kotlinx.coroutines.flow.Flow

data class HomeScreenState(
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
    val userName: String = "",
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
    val artists: List<ResolvedArtistInfo> = emptyList(),
    val year: Int? = null,
)

data class ResolvedArtistInfo(
    val id: String,
    val name: String,
)

enum class ViewedContentType {
    Artist,
    Album,
    Track,
    Playlist,
}
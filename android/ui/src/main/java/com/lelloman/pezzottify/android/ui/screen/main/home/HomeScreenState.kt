package com.lelloman.pezzottify.android.ui.screen.main.home

import com.lelloman.pezzottify.android.ui.component.ConnectionState
import com.lelloman.pezzottify.android.ui.content.Content
import kotlinx.coroutines.flow.Flow

data class HomeScreenState(
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
    val popularContent: PopularContentState? = null,
    val userName: String = "",
    val connectionState: ConnectionState = ConnectionState.Disconnected,
) {
    data class RecentlyViewedContent(
        val contentId: String,
        val contentType: ViewedContentType,
    )
}

/**
 * Popular albums and artists from listening data.
 */
data class PopularContentState(
    val albums: List<PopularAlbumState>,
    val artists: List<PopularArtistState>,
)

data class PopularAlbumState(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val artistNames: List<String>,
)

data class PopularArtistState(
    val id: String,
    val name: String,
    val imageUrl: String?,
)

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
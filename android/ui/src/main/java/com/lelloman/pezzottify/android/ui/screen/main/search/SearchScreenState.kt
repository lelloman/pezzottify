package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.annotation.StringRes
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import kotlinx.coroutines.flow.Flow

data class SearchScreenState(
    val query: String = "",
    val isLoading: Boolean = false,
    val searchResults: List<Flow<Content<SearchResultContent>>>? = null,
    @StringRes val searchErrorRes: Int? = null,
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
    val searchHistoryItems: List<Flow<Content<SearchHistoryItem>>>? = null,
    val selectedFilters: Set<SearchFilter> = emptySet(),
    val canUseExternalSearch: Boolean = false,
    val isExternalMode: Boolean = false,
    val externalResults: List<ExternalSearchResultContent>? = null,
    val externalSearchLoading: Boolean = false,
    @StringRes val externalSearchErrorRes: Int? = null,
    val downloadLimits: UiDownloadLimits? = null,
    val requestingAlbumIds: Set<String> = emptySet(),
    val whatsNewContent: WhatsNewContentState? = null,
)

/**
 * State for the What's New section, showing recently added albums grouped by batch.
 */
data class WhatsNewContentState(
    val albums: List<WhatsNewAlbumGroup>,
    val isLoading: Boolean = false,
)

/**
 * A group of albums from a single batch.
 */
data class WhatsNewAlbumGroup(
    val batchId: String,
    val batchName: String,
    val closedAt: Long,
    val albums: List<Flow<Content<WhatsNewAlbumItem>>>,
)

/**
 * A resolved album item for the What's New widget.
 */
data class WhatsNewAlbumItem(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val artistIds: List<String>,
)

data class SearchHistoryItem(
    val query: String,
    val contentId: String,
    val contentName: String,
    val contentImageUrl: String?,
    val contentType: ViewedContentType,
)
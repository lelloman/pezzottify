package com.lelloman.pezzottify.android.ui.screen.main.search

import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import kotlinx.coroutines.flow.Flow

data class SearchScreenState(
    val query: String = "",
    val isLoading: Boolean = false,
    val searchResults: List<Flow<Content<SearchResultContent>>>? = null,
    val searchError: String? = null,
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
    val searchHistoryItems: List<Flow<Content<SearchHistoryItem>>>? = null,
    val selectedFilters: Set<SearchFilter> = emptySet(),
    val canUseExternalSearch: Boolean = false,
    val isExternalMode: Boolean = false,
)

data class SearchHistoryItem(
    val query: String,
    val contentId: String,
    val contentName: String,
    val contentImageUrl: String?,
    val contentType: ViewedContentType,
)
package com.lelloman.pezzottify.android.ui.screen.main.search

import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import kotlinx.coroutines.flow.Flow

data class SearchScreenState(
    val query: String = "",
    val isLoading: Boolean = false,
    val searchResults: List<Flow<Content<SearchResultContent>>>? = null,
    val searchError: String? = null,
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
)
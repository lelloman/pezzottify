package com.lelloman.pezzottify.android.ui.content

import kotlinx.coroutines.flow.Flow

interface ContentResolver {

    fun resolveSearchResult(itemId: String): Flow<Content<SearchResultContent>>
}
package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import kotlinx.coroutines.flow.Flow

class UiContentResolver : ContentResolver {

    override fun resolveSearchResult(itemId: String): Flow<Content<SearchResultContent>> {
        TODO("Not yet implemented")
    }

}
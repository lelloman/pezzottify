package com.lelloman.pezzottify.android.ui.content

import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.flow.Flow

interface ContentResolver {

    fun resolveSearchResult(
        itemId: String,
        itemType: SearchScreenViewModel.SearchedItemType
    ): Flow<Content<SearchResultContent>>

    fun resolveArtist(artistId: String): Flow<Content<Artist>>

    fun resolveAlbum(albumId: String): Flow<Content<Album>>
}
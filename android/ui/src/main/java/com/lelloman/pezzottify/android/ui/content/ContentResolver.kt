package com.lelloman.pezzottify.android.ui.content

import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.flow.Flow

data class ArtistDiscography(
    val albums: List<String>,
    val features: List<String>,
)

interface ContentResolver {

    fun resolveSearchResult(
        itemId: String,
        itemType: SearchScreenViewModel.SearchedItemType
    ): Flow<Content<SearchResultContent>>

    fun resolveArtist(artistId: String): Flow<Content<Artist>>

    fun resolveAlbum(albumId: String): Flow<Content<Album>>

    fun resolveTrack(trackId: String): Flow<Content<Track>>

    fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>>

    /**
     * Build an image URL from a display image ID.
     */
    fun buildImageUrl(displayImageId: String): String
}
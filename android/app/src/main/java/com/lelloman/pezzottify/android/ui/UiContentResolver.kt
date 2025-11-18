package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class UiContentResolver(private val staticsProvider: StaticsProvider) : ContentResolver {

    override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
        staticsProvider.provideArtist(artistId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, Artist(
                        id = it.id,
                        name = it.data.name,
                    )
                )
            }
        }

    override fun resolveAlbum(albumId: String): Flow<Content<Album>> =
        staticsProvider.provideAlbum(albumId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, Album(
                        id = it.id,
                        name = it.data.name,
                        artistsIds = it.data.artistsIds,
                    )
                )
            }
        }

    override fun resolveTrack(trackId: String): Flow<Content<Track>> =
        staticsProvider.provideTrack(trackId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, Track(
                        id = it.id,
                        name = it.data.name,
                        albumId = it.data.albumId,
                        artistsIds = it.data.artistsIds,
                    )
                )
            }
        }

    override fun resolveSearchResult(
        itemId: String,
        itemType: SearchScreenViewModel.SearchedItemType
    ): Flow<Content<SearchResultContent>> = when (itemType) {
        SearchScreenViewModel.SearchedItemType.Album -> staticsProvider.provideAlbum(itemId)
            .map {
                when (it) {
                    is StaticsItem.Error -> Content.Error(it.id)
                    is StaticsItem.Loading -> Content.Loading(it.id)
                    is StaticsItem.Loaded -> Content.Resolved(
                        it.id, SearchResultContent.Album(
                            id = it.id,
                            name = it.data.name,
                            artistsIds = it.data.artistsIds,
                            imageUrl = "",
                        )
                    )
                }
            }

        SearchScreenViewModel.SearchedItemType.Track -> staticsProvider.provideTrack(itemId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, SearchResultContent.Track(
                        id = it.id,
                        name = it.data.name,
                        artistsIds = it.data.artistsIds,
                        durationSeconds = it.data.durationSeconds,
                        albumId = it.data.albumId,
                    )
                )
            }
        }

        SearchScreenViewModel.SearchedItemType.Artist -> staticsProvider.provideArtist(itemId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, SearchResultContent.Artist(
                        id = it.id,
                        name = it.data.name,
                        imageUrl = "",
                    )
                )
            }
        }
    }
}
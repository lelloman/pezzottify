package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

class UiContentResolver(
    private val staticsProvider: StaticsProvider,
    private val remoteApiClient: RemoteApiClient,
    private val configStore: ConfigStore
) : ContentResolver {

    override fun resolveArtist(artistId: String): Flow<Content<Artist>> =
        staticsProvider.provideArtist(artistId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, Artist(
                        id = it.id,
                        name = it.data.name,
                        imageUrls = ImageUrlProvider.selectImageUrls(
                            configStore.baseUrl.value,
                            it.data.portraits,
                            secondaryImages = it.data.portraitGroup,
                        ),
                        related = it.data.related,
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
                        date = it.data.date,
                        artistsIds = it.data.artistsIds,
                        imageUrls = ImageUrlProvider.selectImageUrls(
                            configStore.baseUrl.value,
                            it.data.covers,
                            secondaryImages = it.data.coverGroup,
                        ),
                        discs = it.data.discs.map { disc ->
                            com.lelloman.pezzottify.android.ui.content.Disc(
                                name = disc.name,
                                tracksIds = disc.tracksIds,
                            )
                        }
                    )
                )
            }
        }

    override fun resolveTrack(trackId: String): Flow<Content<Track>> =
        staticsProvider.provideTrack(trackId).flatMapLatest { trackItem ->
            when (trackItem) {
                is StaticsItem.Error -> flowOf(Content.Error(trackItem.id))
                is StaticsItem.Loading -> flowOf(Content.Loading(trackItem.id))
                is StaticsItem.Loaded -> {
                    val artistFlows = trackItem.data.artistsIds.map { artistId ->
                        staticsProvider.provideArtist(artistId).map { artistItem ->
                            when (artistItem) {
                                is StaticsItem.Loaded -> ArtistInfo(artistId, artistItem.data.name)
                                else -> ArtistInfo(artistId, "")
                            }
                        }
                    }
                    if (artistFlows.isEmpty()) {
                        flowOf(
                            Content.Resolved(
                                trackItem.id, Track(
                                    id = trackItem.id,
                                    name = trackItem.data.name,
                                    albumId = trackItem.data.albumId,
                                    artists = emptyList(),
                                    durationSeconds = trackItem.data.durationSeconds,
                                )
                            )
                        )
                    } else {
                        combine(artistFlows) { artists ->
                            Content.Resolved(
                                trackItem.id, Track(
                                    id = trackItem.id,
                                    name = trackItem.data.name,
                                    albumId = trackItem.data.albumId,
                                    artists = artists.toList(),
                                    durationSeconds = trackItem.data.durationSeconds,
                                )
                            )
                        }
                    }
                }
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
                            imageUrl = ImageUrlProvider.selectImageUrls(
                                baseUrl = configStore.baseUrl.value,
                                primaryImages = it.data.covers,
                                secondaryImages = it.data.coverGroup,
                            ).firstOrNull() ?: "",
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
                        imageUrl = ImageUrlProvider.selectImageUrls(
                            baseUrl = configStore.baseUrl.value,
                            primaryImages = it.data.portraits,
                            secondaryImages = it.data.portraitGroup,
                        ).firstOrNull() ?: "",
                    )
                )
            }
        }
    }

    override suspend fun getArtistDiscography(artistId: String): ArtistDiscography? {
        return when (val response = remoteApiClient.getArtistDiscography(artistId)) {
            is RemoteApiResponse.Success -> ArtistDiscography(
                albums = response.data.albums,
                features = response.data.features
            )

            else -> null
        }
    }
}
package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability as DomainTrackAvailability
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.ArtistDiscography
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.content.TrackAvailability
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

/** Maps domain TrackAvailability to UI TrackAvailability */
private fun DomainTrackAvailability.toUi(): TrackAvailability = when (this) {
    DomainTrackAvailability.Available -> TrackAvailability.Available
    DomainTrackAvailability.Unavailable -> TrackAvailability.Unavailable
    DomainTrackAvailability.Fetching -> TrackAvailability.Fetching
    DomainTrackAvailability.FetchError -> TrackAvailability.FetchError
}

class UiContentResolver(
    private val staticsProvider: StaticsProvider,
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
                        imageUrl = ImageUrlProvider.buildImageUrl(
                            configStore.baseUrl.value,
                            it.data.displayImageId,
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
                        imageUrl = ImageUrlProvider.buildImageUrl(
                            configStore.baseUrl.value,
                            it.data.displayImageId,
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
                                    availability = trackItem.data.availability.toUi(),
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
                                    availability = trackItem.data.availability.toUi(),
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
            .flatMapLatest { albumItem ->
                when (albumItem) {
                    is StaticsItem.Error -> flowOf(Content.Error(albumItem.id))
                    is StaticsItem.Loading -> flowOf(Content.Loading(albumItem.id))
                    is StaticsItem.Loaded -> {
                        val artistFlows = albumItem.data.artistsIds.map { artistId ->
                            staticsProvider.provideArtist(artistId).map { artistItem ->
                                when (artistItem) {
                                    is StaticsItem.Loaded -> artistItem.data.name
                                    else -> ""
                                }
                            }
                        }
                        if (artistFlows.isEmpty()) {
                            flowOf(
                                Content.Resolved(
                                    albumItem.id, SearchResultContent.Album(
                                        id = albumItem.id,
                                        name = albumItem.data.name,
                                        artistNames = emptyList(),
                                        imageUrl = ImageUrlProvider.buildImageUrl(
                                            baseUrl = configStore.baseUrl.value,
                                            displayImageId = albumItem.data.displayImageId,
                                        ),
                                    )
                                )
                            )
                        } else {
                            combine(artistFlows) { artistNames ->
                                Content.Resolved(
                                    albumItem.id, SearchResultContent.Album(
                                        id = albumItem.id,
                                        name = albumItem.data.name,
                                        artistNames = artistNames.toList(),
                                        imageUrl = ImageUrlProvider.buildImageUrl(
                                            baseUrl = configStore.baseUrl.value,
                                            displayImageId = albumItem.data.displayImageId,
                                        ),
                                    )
                                )
                            }
                        }
                    }
                }
            }

        SearchScreenViewModel.SearchedItemType.Track -> staticsProvider.provideTrack(itemId)
            .flatMapLatest { trackItem ->
                when (trackItem) {
                    is StaticsItem.Error -> flowOf(Content.Error(trackItem.id))
                    is StaticsItem.Loading -> flowOf(Content.Loading(trackItem.id))
                    is StaticsItem.Loaded -> {
                        val albumFlow = staticsProvider.provideAlbum(trackItem.data.albumId)
                            .map { albumItem ->
                                when (albumItem) {
                                    is StaticsItem.Loaded -> ImageUrlProvider.buildImageUrl(
                                        baseUrl = configStore.baseUrl.value,
                                        displayImageId = albumItem.data.displayImageId,
                                    )
                                    else -> null
                                }
                            }
                        val artistFlows = trackItem.data.artistsIds.map { artistId ->
                            staticsProvider.provideArtist(artistId).map { artistItem ->
                                when (artistItem) {
                                    is StaticsItem.Loaded -> artistItem.data.name
                                    else -> ""
                                }
                            }
                        }
                        val allFlows = listOf(albumFlow) + artistFlows
                        combine(allFlows) { results ->
                            val albumImageUrl = results[0] as String?
                            val artistNames = results.drop(1).map { it as String }
                            Content.Resolved(
                                trackItem.id, SearchResultContent.Track(
                                    id = trackItem.id,
                                    name = trackItem.data.name,
                                    artistNames = artistNames,
                                    durationSeconds = trackItem.data.durationSeconds,
                                    albumId = trackItem.data.albumId,
                                    albumImageUrl = albumImageUrl,
                                    availability = trackItem.data.availability.toUi(),
                                )
                            )
                        }
                    }
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
                        imageUrl = ImageUrlProvider.buildImageUrl(
                            baseUrl = configStore.baseUrl.value,
                            displayImageId = it.data.displayImageId,
                        ) ?: "",
                    )
                )
            }
        }
    }

    override fun resolveArtistDiscography(artistId: String): Flow<Content<ArtistDiscography>> =
        staticsProvider.provideDiscography(artistId).map {
            when (it) {
                is StaticsItem.Error -> Content.Error(it.id)
                is StaticsItem.Loading -> Content.Loading(it.id)
                is StaticsItem.Loaded -> Content.Resolved(
                    it.id, ArtistDiscography(
                        albums = it.data.albumsIds,
                        features = it.data.featuresIds
                    )
                )
            }
        }

    override fun buildImageUrl(displayImageId: String): String =
        ImageUrlProvider.buildImageUrl(configStore.baseUrl.value, displayImageId)!!
}
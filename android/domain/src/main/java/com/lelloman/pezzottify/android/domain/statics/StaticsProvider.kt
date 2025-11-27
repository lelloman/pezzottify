package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.Synchronizer
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.withContext
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

class StaticsProvider internal constructor(
    private val staticsStore: StaticsStore,
    private val staticItemFetchStateStore: StaticItemFetchStateStore,
    private val synchronizer: Synchronizer,
    loggerFactory: LoggerFactory,
    private val coroutineContext: CoroutineContext,
) {

    @Inject
    internal constructor(
        staticsStore: StaticsStore,
        staticItemFetchStateStore: StaticItemFetchStateStore,
        synchronizer: Synchronizer,
        loggerFactory: LoggerFactory,
    ) : this(staticsStore, staticItemFetchStateStore, synchronizer, loggerFactory, Dispatchers.IO)

    private val logger by loggerFactory

    private suspend fun scheduleItemFetch(itemId: String, type: StaticItemType) {
        withContext(coroutineContext) {
            logger.debug("scheduleItemFetch($itemId, $type)")
            staticItemFetchStateStore.store(StaticItemFetchState.requested(itemId, type))
            synchronizer.wakeUp()
        }
    }

    fun provideArtist(itemId: String): StaticsItemFlow<Artist> {
        return staticsStore.getArtist(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { artist, fetchState ->
                val output = when {
                    artist != null -> StaticsItem.Loaded(
                        itemId,
                        artist
                    )

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        scheduleItemFetch(itemId, StaticItemType.Artist)
                        StaticsItem.Error(
                            itemId,
                            Throwable("${fetchState.errorReason}")
                        )
                    }

                    else -> {
                        scheduleItemFetch(itemId, StaticItemType.Artist)
                        StaticsItem.Loading(itemId)
                    }
                }
                logger.debug("provideArtist($itemId) newOutput = $output")
                output
            }
    }

    fun provideTrack(itemId: String): StaticsItemFlow<Track> {
        return staticsStore.getTrack(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { track, fetchState ->
                val output = when {
                    track != null -> StaticsItem.Loaded(
                        itemId,
                        track
                    )

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        scheduleItemFetch(itemId, StaticItemType.Track)
                        StaticsItem.Error(
                            itemId,
                            Throwable("${fetchState.errorReason}")
                        )
                    }

                    else -> {
                        scheduleItemFetch(itemId, StaticItemType.Track)
                        StaticsItem.Loading(itemId)
                    }
                }
                logger.debug("provideTrack($itemId) newOutput = $output")
                output
            }
    }

    fun provideAlbum(itemId: String): StaticsItemFlow<Album> {
        return staticsStore.getAlbum(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { album, fetchState ->
                val output = when {
                    album != null -> StaticsItem.Loaded(
                        itemId,
                        album
                    )

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        scheduleItemFetch(itemId, StaticItemType.Album)
                        StaticsItem.Error(
                            itemId,
                            Throwable("${fetchState.errorReason}")
                        )
                    }

                    else -> {
                        scheduleItemFetch(itemId, StaticItemType.Album)
                        StaticsItem.Loading(itemId)
                    }
                }
                logger.debug("provideAlbum($itemId) newOutput = $output")
                output
            }
    }

    fun provideDiscography(artistId: String): StaticsItemFlow<ArtistDiscography> {
        return staticsStore.getDiscography(artistId)
            .combine(staticItemFetchStateStore.get(artistId)) { discography, fetchState ->
                val output = when {
                    discography != null -> StaticsItem.Loaded(
                        artistId,
                        discography
                    )

                    fetchState?.isLoading == true -> StaticsItem.Loading(artistId)
                    fetchState?.errorReason != null -> {
                        scheduleItemFetch(artistId, StaticItemType.Discography)
                        StaticsItem.Error(
                            artistId,
                            Throwable("${fetchState.errorReason}")
                        )
                    }

                    else -> {
                        scheduleItemFetch(artistId, StaticItemType.Discography)
                        StaticsItem.Loading(artistId)
                    }
                }
                logger.debug("provideDiscography($artistId) newOutput = $output")
                output
            }
    }
}
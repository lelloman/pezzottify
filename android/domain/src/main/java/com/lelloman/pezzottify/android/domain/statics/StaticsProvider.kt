package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollector
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.StaticsSynchronizer
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

class StaticsProvider internal constructor(
    private val staticsStore: StaticsStore,
    private val staticItemFetchStateStore: StaticItemFetchStateStore,
    private val staticsSynchronizer: StaticsSynchronizer,
    private val timeProvider: TimeProvider,
    private val staticsCache: StaticsCache,
    private val cacheMetricsCollector: CacheMetricsCollector,
    private val userSettingsStore: UserSettingsStore,
    private val skeletonStore: SkeletonStore,
    loggerFactory: LoggerFactory,
    private val coroutineContext: CoroutineContext,
) {

    @Inject
    internal constructor(
        staticsStore: StaticsStore,
        staticItemFetchStateStore: StaticItemFetchStateStore,
        staticsSynchronizer: StaticsSynchronizer,
        timeProvider: TimeProvider,
        staticsCache: StaticsCache,
        cacheMetricsCollector: CacheMetricsCollector,
        userSettingsStore: UserSettingsStore,
        skeletonStore: SkeletonStore,
        loggerFactory: LoggerFactory,
    ) : this(
        staticsStore,
        staticItemFetchStateStore,
        staticsSynchronizer,
        timeProvider,
        staticsCache,
        cacheMetricsCollector,
        userSettingsStore,
        skeletonStore,
        loggerFactory,
        Dispatchers.IO
    )

    private val logger by loggerFactory

    private val isCacheEnabled: Boolean
        get() = userSettingsStore.isInMemoryCacheEnabled.value

    private fun StaticItemFetchState.isBackoffExpired(): Boolean {
        val tryNext = tryNextTime ?: return true
        return tryNext <= timeProvider.nowUtcMs()
    }

    private suspend fun scheduleItemFetch(itemId: String, type: StaticItemType) {
        withContext(coroutineContext) {
            logger.debug("scheduleItemFetch($itemId, $type)")
            staticItemFetchStateStore.store(StaticItemFetchState.requested(itemId, type))
            staticsSynchronizer.wakeUp()
        }
    }

    fun provideArtist(itemId: String): StaticsItemFlow<Artist> {
        // Check in-memory cache first if enabled
        if (isCacheEnabled) {
            staticsCache.artistCache.get(itemId)?.let { cached ->
                cacheMetricsCollector.recordCacheHit("artist")
                logger.debug("provideArtist($itemId) cache hit")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            cacheMetricsCollector.recordCacheMiss("artist")
        }

        // Fall back to database flow
        return staticsStore.getArtist(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { artist, fetchState ->
                val output = when {
                    artist != null -> {
                        // Cache successful loads if enabled
                        if (isCacheEnabled) {
                            staticsCache.artistCache.put(itemId, artist)
                        }
                        StaticsItem.Loaded(itemId, artist)
                    }

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        if (fetchState.isBackoffExpired()) {
                            scheduleItemFetch(itemId, StaticItemType.Artist)
                        }
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
                if (output is StaticsItem.Error) {
                    output.error.printStackTrace()
                    logger.warn("Error providing artist", output.error)
                }
                logger.debug("provideArtist($itemId) newOutput = $output")
                output
            }
    }

    fun provideTrack(itemId: String): StaticsItemFlow<Track> {
        // Check in-memory cache first if enabled
        if (isCacheEnabled) {
            staticsCache.trackCache.get(itemId)?.let { cached ->
                cacheMetricsCollector.recordCacheHit("track")
                logger.debug("provideTrack($itemId) cache hit")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            cacheMetricsCollector.recordCacheMiss("track")
        }

        // Fall back to database flow
        return staticsStore.getTrack(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { track, fetchState ->
                val output = when {
                    track != null -> {
                        // Cache successful loads if enabled
                        if (isCacheEnabled) {
                            staticsCache.trackCache.put(itemId, track)
                        }
                        StaticsItem.Loaded(itemId, track)
                    }

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        if (fetchState.isBackoffExpired()) {
                            scheduleItemFetch(itemId, StaticItemType.Track)
                        }
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
        // Check in-memory cache first if enabled
        if (isCacheEnabled) {
            staticsCache.albumCache.get(itemId)?.let { cached ->
                cacheMetricsCollector.recordCacheHit("album")
                logger.debug("provideAlbum($itemId) cache hit")
                return flowOf(StaticsItem.Loaded(itemId, cached))
            }
            cacheMetricsCollector.recordCacheMiss("album")
        }

        // Fall back to database flow
        return staticsStore.getAlbum(itemId)
            .combine(staticItemFetchStateStore.get(itemId)) { album, fetchState ->
                val output = when {
                    album != null -> {
                        // Cache successful loads if enabled
                        if (isCacheEnabled) {
                            staticsCache.albumCache.put(itemId, album)
                        }
                        StaticsItem.Loaded(itemId, album)
                    }

                    fetchState?.isLoading == true -> StaticsItem.Loading(itemId)
                    fetchState?.errorReason != null -> {
                        if (fetchState.isBackoffExpired()) {
                            scheduleItemFetch(itemId, StaticItemType.Album)
                        }
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
        // Skeleton is the source of truth for artist-album relationships
        return skeletonStore.observeAlbumIdsForArtist(artistId).map { skeletonAlbumIds ->
            val output = if (skeletonAlbumIds.isNotEmpty()) {
                logger.debug("provideDiscography($artistId) skeleton has ${skeletonAlbumIds.size} albums")
                StaticsItem.Loaded(
                    artistId,
                    object : ArtistDiscography {
                        override val artistId = artistId
                        override val albumsIds = skeletonAlbumIds
                        override val featuresIds = emptyList<String>()
                    }
                )
            } else {
                // No skeleton data yet - show loading (skeleton sync should populate this)
                logger.debug("provideDiscography($artistId) no skeleton data, showing loading")
                StaticsItem.Loading(artistId)
            }
            output
        }
    }

    fun clearCache() {
        staticsCache.clearAll()
        logger.debug("Cache cleared")
    }
}
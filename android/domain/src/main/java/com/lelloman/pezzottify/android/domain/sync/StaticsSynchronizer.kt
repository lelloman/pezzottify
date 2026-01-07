package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.toDomain
import com.lelloman.pezzottify.android.domain.statics.StaticItemType
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.coroutineContext
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@Singleton
internal class StaticsSynchronizer(
    private val fetchStateStore: StaticItemFetchStateStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<StaticItemFetchState>(
    logger = loggerFactory.getLogger(StaticsSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    @Inject
    constructor(
        fetchStateStore: StaticItemFetchStateStore,
        remoteApiClient: RemoteApiClient,
        staticsStore: StaticsStore,
        timeProvider: TimeProvider,
        loggerFactory: LoggerFactory,
    ) : this(
        fetchStateStore,
        remoteApiClient,
        staticsStore,
        timeProvider,
        loggerFactory,
        Dispatchers.IO,
        GlobalScope
    )

    override suspend fun onBeforeMainLoop() {
        fetchStateStore.resetLoadingStates()
    }

    override suspend fun getItemsToProcess(): List<StaticItemFetchState> {
        return fetchStateStore.getIdle()
    }

    override suspend fun shouldContinueWhenNoItems(): Boolean {
        val loadingCount = fetchStateStore.getLoadingItemsCount()
        logger.debug("shouldContinueWhenNoItems() loadingCount=$loadingCount")
        return loadingCount > 0
    }

    override suspend fun processItem(item: StaticItemFetchState) {
        fetchItemFromRemote(item.itemId, item.itemType)
    }

    private suspend fun fetchItemFromRemote(itemId: String, type: StaticItemType) {
        withContext(coroutineContext) {
            val attemptTime = timeProvider.nowUtcMs()
            val loadingState = StaticItemFetchState.loading(itemId, type, attemptTime)
            fetchStateStore.store(loadingState)
            val remoteData = when (type) {
                StaticItemType.Album -> remoteApiClient.getAlbum(itemId)
                StaticItemType.Artist -> remoteApiClient.getArtist(itemId)
                StaticItemType.Track -> remoteApiClient.getTrack(itemId)
                StaticItemType.Discography -> remoteApiClient.getArtistDiscography(itemId)
            }
            if (remoteData is RemoteApiResponse.Success) {
                try {
                    when (remoteData.data) {
                        is AlbumResponse -> staticsStore.storeAlbum(remoteData.data.toDomain())
                        is ArtistResponse -> staticsStore.storeArtist(remoteData.data.toDomain())
                        is TrackResponse -> staticsStore.storeTrack(remoteData.data.toDomain())
                        is ArtistDiscographyResponse -> {
                            val allAlbums = fetchAllDiscographyPages(itemId, remoteData.data)
                            staticsStore.storeDiscography(allAlbums.toDomain(itemId))
                        }
                        else -> logger.error("Cannot store unknown response data of type ${remoteData.javaClass} -> ${remoteData.data}")
                    }
                    fetchStateStore.delete(itemId)
                    logger.debug("Fetched and stored data for $itemId: ${remoteData.data}")
                } catch (throwable: Throwable) {
                    logger.error(
                        "Error while storing remote-fetched data into StaticsStore",
                        throwable
                    )
                    val tryNextTime = attemptTime + RETRY_DELAY_CLIENT_ERROR_MS
                    fetchStateStore.store(
                        StaticItemFetchState.error(
                            itemId = itemId,
                            itemType = type,
                            errorReason = ErrorReason.Client,
                            lastAttemptTime = attemptTime,
                            tryNextTime = tryNextTime
                        )
                    )
                }
            } else {
                logger.debug("Remote API returned error: $remoteData")
                val (errorReason, retryDelayMs) = mapErrorToReasonAndDelay(remoteData)
                val tryNextTime = attemptTime + retryDelayMs
                fetchStateStore.store(
                    StaticItemFetchState.error(
                        itemId = itemId,
                        itemType = type,
                        errorReason = errorReason,
                        lastAttemptTime = attemptTime,
                        tryNextTime = tryNextTime
                    )
                )
            }
        }
    }

    private fun mapErrorToReasonAndDelay(error: RemoteApiResponse<*>): Pair<ErrorReason, Long> {
        return when (error) {
            is RemoteApiResponse.Error.Network -> ErrorReason.Network to RETRY_DELAY_NETWORK_ERROR_MS
            is RemoteApiResponse.Error.Unauthorized -> ErrorReason.Client to RETRY_DELAY_UNAUTHORIZED_ERROR_MS
            is RemoteApiResponse.Error.NotFound -> ErrorReason.NotFound to RETRY_DELAY_NOT_FOUND_ERROR_MS
            is RemoteApiResponse.Error.Unknown -> ErrorReason.Unknown to RETRY_DELAY_UNKNOWN_ERROR_MS
            else -> ErrorReason.Unknown to RETRY_DELAY_UNKNOWN_ERROR_MS
        }
    }

    private companion object {
        val MIN_SLEEP_DURATION = 5.milliseconds
        val MAX_SLEEP_DURATION = 10.seconds

        // Retry delay constants in milliseconds
        const val RETRY_DELAY_NETWORK_ERROR_MS = 60_000L // 1 minute for network errors
        const val RETRY_DELAY_UNAUTHORIZED_ERROR_MS = 1_800_000L // 30 minutes for 403/unauthorized
        const val RETRY_DELAY_NOT_FOUND_ERROR_MS = 3_600_000L // 1 hour for 404/not found
        const val RETRY_DELAY_UNKNOWN_ERROR_MS = 300_000L // 5 minutes for unknown errors
        const val RETRY_DELAY_CLIENT_ERROR_MS = 300_000L // 5 minutes for client errors

        // Pagination constants
        const val DISCOGRAPHY_PAGE_SIZE = 50
        const val DISCOGRAPHY_MAX_ALBUMS_TO_FETCH = 500 // Limit for initial fetch
    }

    /**
     * Fetch all pages of an artist's discography and combine them.
     * Limits total albums fetched to avoid overwhelming the system.
     */
    private suspend fun fetchAllDiscographyPages(
        artistId: String,
        initialResponse: com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
    ): com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse {
        var allAlbums = initialResponse.albums.toMutableList()
        var offset = initialResponse.offset ?: 0
        var limit = initialResponse.limit ?: DISCOGRAPHY_PAGE_SIZE
        var hasMore = initialResponse.hasMore

        while (hasMore && allAlbums.size < DISCOGRAPHY_MAX_ALBUMS_TO_FETCH) {
            val nextResponse = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset + limit,
                limit = DISCOGRAPHY_PAGE_SIZE
            )

            when (nextResponse) {
                is RemoteApiResponse.Success -> {
                    allAlbums.addAll(nextResponse.data.albums)
                    offset += limit
                    limit = DISCOGRAPHY_PAGE_SIZE
                    hasMore = nextResponse.data.hasMore
                }
                else -> {
                    logger.warn("Failed to fetch discography page for $artistId: $nextResponse")
                    break
                }
            }
        }

        return initialResponse.copy(
            albums = allAlbums,
            total = initialResponse.total,
            hasMore = hasMore && allAlbums.size < initialResponse.total
        )
    }
}

package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.request.BatchContentRequest
import com.lelloman.pezzottify.android.domain.remoteapi.request.BatchItemRequest
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.BatchItemResult
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.toDomain
import com.lelloman.pezzottify.android.domain.skeleton.AlbumArtistRelationship
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.BatchBaseSynchronizer
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@Singleton
internal class StaticsSynchronizer(
    private val fetchStateStore: StaticItemFetchStateStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val skeletonStore: SkeletonStore,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BatchBaseSynchronizer<StaticItemFetchState>(
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
        skeletonStore: SkeletonStore,
        timeProvider: TimeProvider,
        loggerFactory: LoggerFactory,
    ) : this(
        fetchStateStore,
        remoteApiClient,
        staticsStore,
        skeletonStore,
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

    override suspend fun processBatch(items: List<StaticItemFetchState>) {
        if (items.isEmpty()) return

        val attemptTime = timeProvider.nowUtcMs()

        // Separate batchable items (Artist, Album, Track) from non-batchable (Discography)
        val batchableItems = items.filter { it.itemType != StaticItemType.Discography }
        val discographyItems = items.filter { it.itemType == StaticItemType.Discography }

        // Mark all items as loading
        items.forEach { item ->
            val loadingState = StaticItemFetchState.Companion.loading(item.itemId, item.itemType, attemptTime)
            fetchStateStore.store(loadingState)
        }

        // Process batchable items via batch endpoint
        if (batchableItems.isNotEmpty()) {
            processBatchableItems(batchableItems, attemptTime)
        }

        // Process discography items individually (they require pagination)
        discographyItems.forEach { item ->
            processDiscographyItem(item.itemId, attemptTime)
        }
    }

    private suspend fun processBatchableItems(items: List<StaticItemFetchState>, attemptTime: Long) {
        val artistItems = items.filter { it.itemType == StaticItemType.Artist }
        val albumItems = items.filter { it.itemType == StaticItemType.Album }
        val trackItems = items.filter { it.itemType == StaticItemType.Track }

        val request = BatchContentRequest(
            artists = artistItems.map { BatchItemRequest(it.itemId, resolved = true) },
            albums = albumItems.map { BatchItemRequest(it.itemId, resolved = true) },
            tracks = trackItems.map { BatchItemRequest(it.itemId, resolved = true) },
        )

        logger.debug("Fetching batch: ${artistItems.size} artists, ${albumItems.size} albums, ${trackItems.size} tracks")

        when (val response = remoteApiClient.getBatchContent(request)) {
            is RemoteApiResponse.Success -> {
                val batchResponse = response.data

                // Process artist results
                artistItems.forEach { item ->
                    val result = batchResponse.artists[item.itemId]
                    processArtistResult(item.itemId, result, attemptTime)
                }

                // Process album results
                albumItems.forEach { item ->
                    val result = batchResponse.albums[item.itemId]
                    processAlbumResult(item.itemId, result, attemptTime)
                }

                // Process track results
                trackItems.forEach { item ->
                    val result = batchResponse.tracks[item.itemId]
                    processTrackResult(item.itemId, result, attemptTime)
                }
            }
            is RemoteApiResponse.Error -> {
                // Batch request failed - mark all items as error
                logger.error("Batch request failed: $response")
                val (errorReason, retryDelayMs) = mapErrorToReasonAndDelay(response)
                val tryNextTime = attemptTime + retryDelayMs

                items.forEach { item ->
                    fetchStateStore.store(
                        StaticItemFetchState.Companion.error(
                            itemId = item.itemId,
                            itemType = item.itemType,
                            errorReason = errorReason,
                            lastAttemptTime = attemptTime,
                            tryNextTime = tryNextTime
                        )
                    )
                }
            }
        }
    }

    private suspend fun processArtistResult(
        itemId: String,
        result: BatchItemResult<com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse>?,
        attemptTime: Long
    ) {
        when (result) {
            is BatchItemResult.Ok -> {
                try {
                    staticsStore.storeArtist(result.value.toDomain())
                    fetchStateStore.delete(itemId)
                    logger.debug("Stored artist $itemId from batch")
                } catch (throwable: Throwable) {
                    logger.error("Error storing artist $itemId", throwable)
                    storeClientError(itemId, StaticItemType.Artist, attemptTime)
                }
            }
            is BatchItemResult.Error -> {
                logger.debug("Batch artist $itemId error: ${result.error}")
                val (errorReason, retryDelayMs) = mapBatchErrorToReasonAndDelay(result.error)
                fetchStateStore.store(
                    StaticItemFetchState.Companion.error(
                        itemId = itemId,
                        itemType = StaticItemType.Artist,
                        errorReason = errorReason,
                        lastAttemptTime = attemptTime,
                        tryNextTime = attemptTime + retryDelayMs
                    )
                )
            }
            null -> {
                logger.warn("Artist $itemId not in batch response")
                storeClientError(itemId, StaticItemType.Artist, attemptTime)
            }
        }
    }

    private suspend fun processAlbumResult(
        itemId: String,
        result: BatchItemResult<com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse>?,
        attemptTime: Long
    ) {
        when (result) {
            is BatchItemResult.Ok -> {
                try {
                    staticsStore.storeAlbum(result.value.toDomain())
                    fetchStateStore.delete(itemId)
                    logger.debug("Stored album $itemId from batch")
                } catch (throwable: Throwable) {
                    logger.error("Error storing album $itemId", throwable)
                    storeClientError(itemId, StaticItemType.Album, attemptTime)
                }
            }
            is BatchItemResult.Error -> {
                logger.debug("Batch album $itemId error: ${result.error}")
                val (errorReason, retryDelayMs) = mapBatchErrorToReasonAndDelay(result.error)
                fetchStateStore.store(
                    StaticItemFetchState.Companion.error(
                        itemId = itemId,
                        itemType = StaticItemType.Album,
                        errorReason = errorReason,
                        lastAttemptTime = attemptTime,
                        tryNextTime = attemptTime + retryDelayMs
                    )
                )
            }
            null -> {
                logger.warn("Album $itemId not in batch response")
                storeClientError(itemId, StaticItemType.Album, attemptTime)
            }
        }
    }

    private suspend fun processTrackResult(
        itemId: String,
        result: BatchItemResult<com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse>?,
        attemptTime: Long
    ) {
        when (result) {
            is BatchItemResult.Ok -> {
                try {
                    staticsStore.storeTrack(result.value.toDomain())
                    fetchStateStore.delete(itemId)
                    logger.debug("Stored track $itemId from batch")
                } catch (throwable: Throwable) {
                    logger.error("Error storing track $itemId", throwable)
                    storeClientError(itemId, StaticItemType.Track, attemptTime)
                }
            }
            is BatchItemResult.Error -> {
                logger.debug("Batch track $itemId error: ${result.error}")
                val (errorReason, retryDelayMs) = mapBatchErrorToReasonAndDelay(result.error)
                fetchStateStore.store(
                    StaticItemFetchState.Companion.error(
                        itemId = itemId,
                        itemType = StaticItemType.Track,
                        errorReason = errorReason,
                        lastAttemptTime = attemptTime,
                        tryNextTime = attemptTime + retryDelayMs
                    )
                )
            }
            null -> {
                logger.warn("Track $itemId not in batch response")
                storeClientError(itemId, StaticItemType.Track, attemptTime)
            }
        }
    }

    private suspend fun storeClientError(itemId: String, itemType: StaticItemType, attemptTime: Long) {
        fetchStateStore.store(
            StaticItemFetchState.Companion.error(
                itemId = itemId,
                itemType = itemType,
                errorReason = ErrorReason.Client,
                lastAttemptTime = attemptTime,
                tryNextTime = attemptTime + RETRY_DELAY_CLIENT_ERROR_MS
            )
        )
    }

    private suspend fun processDiscographyItem(artistId: String, attemptTime: Long) {
        when (val response = remoteApiClient.getArtistDiscography(artistId)) {
            is RemoteApiResponse.Success -> {
                try {
                    val allAlbums = fetchAllDiscographyPages(artistId, response.data)
                    val albumArtists = allAlbums.albums.map { album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)
                    fetchStateStore.delete(artistId)
                    logger.debug("Stored discography for $artistId")
                } catch (throwable: Throwable) {
                    logger.error("Error storing discography for $artistId", throwable)
                    storeClientError(artistId, StaticItemType.Discography, attemptTime)
                }
            }
            is RemoteApiResponse.Error -> {
                logger.debug("Discography fetch for $artistId failed: $response")
                val (errorReason, retryDelayMs) = mapErrorToReasonAndDelay(response)
                fetchStateStore.store(
                    StaticItemFetchState.Companion.error(
                        itemId = artistId,
                        itemType = StaticItemType.Discography,
                        errorReason = errorReason,
                        lastAttemptTime = attemptTime,
                        tryNextTime = attemptTime + retryDelayMs
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

    private fun mapBatchErrorToReasonAndDelay(error: String): Pair<ErrorReason, Long> {
        return when (error) {
            "not_found" -> ErrorReason.NotFound to RETRY_DELAY_NOT_FOUND_ERROR_MS
            "internal_error" -> ErrorReason.Unknown to RETRY_DELAY_UNKNOWN_ERROR_MS
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
        initialResponse: ArtistDiscographyResponse
    ): ArtistDiscographyResponse {
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
package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
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
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.coroutineContext
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@Singleton
internal class Synchronizer(
    private val fetchStateStore: StaticItemFetchStateStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
    private val dispatcher: CoroutineDispatcher,
    private val scope: CoroutineScope,
) : AppInitializer {

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

    private var initialized = false
    private var sleepDuration = 10.milliseconds
    private var wakeUpSignal = CompletableDeferred<Unit>()

    private val logger by loggerFactory

    override fun initialize() {
        scope.launch(dispatcher) {
            if (!initialized) {
                initialized = true
                mainLoop()
            }
        }
    }

    fun wakeUp() {
        logger.info("wakeUp()")
        wakeUpSignal.complete(Unit)
        sleepDuration = MIN_SLEEP_DURATION
    }

    private suspend fun mainLoop() {
        fetchStateStore.resetLoadingStates()
        while (true) {
            logger.info("mainLoop() iteration")
            val itemsToFetch = fetchStateStore.getIdle()
            val loadingCount = fetchStateStore.getLoadingItemsCount()
            logger.debug("mainLoop() got ${itemsToFetch.size} items to fetch and $loadingCount loading")
            if (itemsToFetch.isEmpty() && loadingCount == 0) {
                logger.info("mainLoop() going to sleep")
                wakeUpSignal.await()
                wakeUpSignal = CompletableDeferred()
                sleepDuration = MIN_SLEEP_DURATION
                continue
            }
            itemsToFetch.forEach { item -> fetchItemFromRemote(item.itemId, item.itemType) }

            logger.debug("mainLoop() going to wait for $sleepDuration")
            delay(sleepDuration)
            sleepDuration *= 1.4
            if (sleepDuration > MAX_SLEEP_DURATION) {
                sleepDuration = MAX_SLEEP_DURATION
            }
        }
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
            }
            if (remoteData is RemoteApiResponse.Success) {
                try {
                    when (remoteData.data) {
                        is AlbumResponse -> staticsStore.storeAlbum(remoteData.data.toDomain())
                        is ArtistResponse -> staticsStore.storeArtist(remoteData.data.toDomain())
                        is TrackResponse -> staticsStore.storeTrack(remoteData.data.toDomain())
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
    }
}
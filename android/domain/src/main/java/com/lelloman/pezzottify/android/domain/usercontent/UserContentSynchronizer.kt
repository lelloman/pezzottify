package com.lelloman.pezzottify.android.domain.usercontent

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@Singleton
class UserContentSynchronizer internal constructor(
    private val userContentStore: UserContentStore,
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
    private val dispatcher: CoroutineDispatcher,
    private val scope: CoroutineScope,
) : AppInitializer {

    @Inject
    constructor(
        userContentStore: UserContentStore,
        remoteApiClient: RemoteApiClient,
        loggerFactory: LoggerFactory,
    ) : this(
        userContentStore,
        remoteApiClient,
        loggerFactory,
        Dispatchers.IO,
        GlobalScope
    )

    private var initialized = false
    private var sleepDuration = MIN_SLEEP_DURATION
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
        while (true) {
            logger.debug("mainLoop() iteration")
            val pendingItems = userContentStore.getPendingSyncItems().first()
            logger.debug("mainLoop() got ${pendingItems.size} pending items")

            if (pendingItems.isEmpty()) {
                logger.info("mainLoop() going to sleep")
                wakeUpSignal.await()
                wakeUpSignal = CompletableDeferred()
                sleepDuration = MIN_SLEEP_DURATION
                continue
            }

            for (item in pendingItems) {
                syncItem(item)
            }

            logger.debug("mainLoop() going to wait for $sleepDuration")
            delay(sleepDuration)
            sleepDuration *= 1.4
            if (sleepDuration > MAX_SLEEP_DURATION) {
                sleepDuration = MAX_SLEEP_DURATION
            }
        }
    }

    private suspend fun syncItem(item: LikedContent) {
        logger.debug("syncItem() syncing ${item.contentId}, isLiked=${item.isLiked}")
        userContentStore.updateSyncStatus(item.contentId, SyncStatus.Syncing)

        val result = if (item.isLiked) {
            remoteApiClient.likeContent(item.contentId)
        } else {
            remoteApiClient.unlikeContent(item.contentId)
        }

        when (result) {
            is RemoteApiResponse.Success -> {
                logger.info("syncItem() success for ${item.contentId}")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.Synced)
            }
            is RemoteApiResponse.Error.Network -> {
                logger.debug("syncItem() network error for ${item.contentId}, will retry later")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.PendingSync)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("syncItem() unauthorized for ${item.contentId}")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.NotFound -> {
                logger.warn("syncItem() not found for ${item.contentId}")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.Unknown -> {
                logger.error("syncItem() unknown error for ${item.contentId}: ${result.message}")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.SyncError)
            }
        }
    }

    private companion object {
        val MIN_SLEEP_DURATION = 100.milliseconds
        val MAX_SLEEP_DURATION = 30.seconds
    }
}

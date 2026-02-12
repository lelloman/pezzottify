package com.lelloman.pezzottify.android.domain.usercontent

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@Singleton
class UserContentSynchronizer @Inject constructor(
    private val userContentStore: UserContentStore,
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<LikedContent>(
    logger = loggerFactory.getLogger(UserContentSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    override suspend fun getItemsToProcess(): List<LikedContent> {
        return userContentStore.getPendingSyncItems().first()
    }

    override suspend fun processItem(item: LikedContent) {
        syncItem(item)
    }

    suspend fun fetchRemoteLikedContent() {
        logger.info("fetchRemoteLikedContent() starting")

        val contentTypes = listOf(
            LikedContent.ContentType.Album to "album",
            LikedContent.ContentType.Artist to "artist",
        )

        for ((type, apiType) in contentTypes) {
            when (val result = remoteApiClient.getLikedContent(apiType)) {
                is RemoteApiResponse.Success -> {
                    val remoteIds = result.data.toSet()
                    logger.info("fetchRemoteLikedContent() got ${remoteIds.size} remote $apiType items")

                    // Get current local items of this type
                    val localItems = userContentStore.getLikedContent(listOf(type)).first()
                    val localIds = localItems.associate { it.contentId to it }

                    // Add remote items that aren't in local DB
                    for (remoteId in remoteIds) {
                        val localItem = localIds[remoteId]
                        if (localItem == null) {
                            // Not in local DB, add as synced
                            logger.debug("fetchRemoteLikedContent() adding remote item $remoteId")
                            userContentStore.setLiked(
                                contentId = remoteId,
                                type = type,
                                liked = true,
                                modifiedAt = System.currentTimeMillis(),
                                syncStatus = SyncStatus.Synced,
                            )
                        }
                        // If local item exists, keep it (local state is authoritative for pending items)
                    }

                    // Mark local "liked" items not on server as pending sync
                    // (they were liked offline and need to be synced)
                    for (localItem in localItems) {
                        if (localItem.isLiked &&
                            localItem.syncStatus == SyncStatus.Synced &&
                            localItem.contentId !in remoteIds
                        ) {
                            // Local says liked + synced, but server doesn't have it
                            // This means it was unliked on another device, update local
                            logger.debug("fetchRemoteLikedContent() removing unliked item ${localItem.contentId}")
                            userContentStore.setLiked(
                                contentId = localItem.contentId,
                                type = type,
                                liked = false,
                                modifiedAt = System.currentTimeMillis(),
                                syncStatus = SyncStatus.Synced,
                            )
                        }
                    }
                }
                is RemoteApiResponse.Error.Unauthorized -> {
                    logger.warn("fetchRemoteLikedContent() unauthorized for $apiType")
                }
                is RemoteApiResponse.Error.Network -> {
                    logger.warn("fetchRemoteLikedContent() network error for $apiType, will retry on next sync")
                }
                else -> {
                    logger.error("fetchRemoteLikedContent() error for $apiType: $result")
                }
            }
        }
        logger.info("fetchRemoteLikedContent() done")
    }

    private suspend fun syncItem(item: LikedContent) {
        logger.debug("syncItem() syncing ${item.contentId}, isLiked=${item.isLiked}")
        userContentStore.updateSyncStatus(item.contentId, SyncStatus.Syncing)

        val contentType = when (item.contentType) {
            LikedContent.ContentType.Album -> "album"
            LikedContent.ContentType.Artist -> "artist"
            LikedContent.ContentType.Track -> "track"
        }

        val result = if (item.isLiked) {
            remoteApiClient.likeContent(contentType, item.contentId)
        } else {
            remoteApiClient.unlikeContent(contentType, item.contentId)
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
            is RemoteApiResponse.Error.EventsPruned -> {
                // EventsPruned is not expected for like/unlike operations
                logger.error("syncItem() unexpected events pruned error for ${item.contentId}")
                userContentStore.updateSyncStatus(item.contentId, SyncStatus.SyncError)
            }
        }
    }

    private companion object {
        val MIN_SLEEP_DURATION = 100.milliseconds
        val MAX_SLEEP_DURATION = 30.seconds
    }
}

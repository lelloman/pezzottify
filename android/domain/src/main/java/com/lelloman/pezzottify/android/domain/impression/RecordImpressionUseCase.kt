package com.lelloman.pezzottify.android.domain.impression

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Records an impression (page view) for content.
 * Saves to local database and wakes up the synchronizer to send to server.
 */
@Singleton
class RecordImpressionUseCase @Inject constructor(
    private val impressionStore: ImpressionStore,
    private val impressionSynchronizer: ImpressionSynchronizer,
    private val timeProvider: TimeProvider,
    private val coroutineScope: CoroutineScope,
) {

    operator fun invoke(itemId: String, itemType: ItemType) {
        coroutineScope.launch {
            val impression = Impression(
                itemId = itemId,
                itemType = itemType,
                syncStatus = SyncStatus.PendingSync,
                createdAt = timeProvider.nowUtcMs(),
            )
            impressionStore.saveImpression(impression)
            impressionSynchronizer.wakeUp()
        }
    }

    fun recordArtist(artistId: String) = invoke(artistId, ItemType.Artist)
    fun recordAlbum(albumId: String) = invoke(albumId, ItemType.Album)
    fun recordTrack(trackId: String) = invoke(trackId, ItemType.Track)
}

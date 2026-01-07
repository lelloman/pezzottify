package com.lelloman.pezzottify.android.localdata.internal.impression

import com.lelloman.pezzottify.android.domain.impression.Impression
import com.lelloman.pezzottify.android.domain.impression.ImpressionStore
import com.lelloman.pezzottify.android.domain.impression.ItemType
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class ImpressionStoreImpl @Inject constructor(
    private val dao: ImpressionDao,
) : ImpressionStore {

    override suspend fun saveImpression(impression: Impression): Long {
        return dao.insert(impression.toEntity())
    }

    override suspend fun getPendingSyncImpressions(): List<Impression> =
        dao.getPendingSync().map { it.toDomain() }

    override suspend fun updateSyncStatus(id: Long, status: SyncStatus) {
        dao.updateSyncStatus(id, status.name)
    }

    override suspend fun deleteImpression(id: Long) {
        dao.delete(id)
    }

    override suspend fun deleteOldNonSyncedImpressions(olderThanMs: Long): Int =
        dao.deleteOldNonSynced(olderThanMs)

    override suspend fun deleteSyncedImpressions(): Int =
        dao.deleteSynced()

    override suspend fun deleteAll() {
        dao.deleteAll()
    }

    private fun Impression.toEntity() = ImpressionEntity(
        id = id,
        itemId = itemId,
        itemType = itemType.name.lowercase(),
        syncStatus = syncStatus.name,
        createdAt = createdAt,
    )

    private fun ImpressionEntity.toDomain() = Impression(
        id = id,
        itemId = itemId,
        itemType = ItemType.fromString(itemType),
        syncStatus = SyncStatus.valueOf(syncStatus),
        createdAt = createdAt,
    )
}

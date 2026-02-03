package com.lelloman.pezzottify.android.localdata.internal.usercontent

import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.toEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

internal class UserContentStoreImpl(
    private val likedContentDao: LikedContentDao,
) : UserContentStore {

    override fun isLiked(contentId: String): Flow<Boolean> =
        likedContentDao.getByContentId(contentId).map { it?.isLiked == true }

    override fun getLikedContent(types: List<LikedContent.ContentType>?): Flow<List<LikedContent>> =
        if (types == null) {
            likedContentDao.getAllLiked()
        } else {
            likedContentDao.getLikedByTypes(types.map { it.name })
        }

    override suspend fun setLiked(
        contentId: String,
        type: LikedContent.ContentType,
        liked: Boolean,
        modifiedAt: Long,
        syncStatus: SyncStatus,
    ) {
        likedContentDao.upsert(
            LikedContentEntity(
                contentId = contentId,
                contentType = type,
                isLiked = liked,
                modifiedAt = modifiedAt,
                syncStatus = syncStatus,
            )
        )
    }

    override suspend fun updateSyncStatus(contentId: String, status: SyncStatus) {
        likedContentDao.updateSyncStatus(contentId, status.name)
    }

    override fun getPendingSyncItems(): Flow<List<LikedContent>> =
        likedContentDao.getPendingSync()

    override suspend fun replaceAllLikedContent(items: List<LikedContent>) {
        likedContentDao.replaceAll(items.map { it.toEntity() })
    }

    override suspend fun deleteAll() {
        likedContentDao.deleteAll()
    }
}

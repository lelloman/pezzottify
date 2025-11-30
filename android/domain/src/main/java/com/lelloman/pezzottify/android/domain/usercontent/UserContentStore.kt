package com.lelloman.pezzottify.android.domain.usercontent

import kotlinx.coroutines.flow.Flow

interface UserContentStore {

    fun isLiked(contentId: String): Flow<Boolean>

    fun getLikedContent(types: List<LikedContent.ContentType>? = null): Flow<List<LikedContent>>

    suspend fun setLiked(
        contentId: String,
        type: LikedContent.ContentType,
        liked: Boolean,
        modifiedAt: Long,
    )

    suspend fun updateSyncStatus(contentId: String, status: SyncStatus)

    fun getPendingSyncItems(): Flow<List<LikedContent>>

    suspend fun replaceAllLikedContent(items: List<LikedContent>)

    suspend fun deleteAll()
}

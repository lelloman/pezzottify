package com.lelloman.pezzottify.android.localdata.internal.user

import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.localdata.internal.user.model.dbValue
import com.lelloman.pezzottify.android.localdata.internal.user.model.toLocalData
import kotlinx.coroutines.flow.Flow

internal class UserDataStoreImpl(
    private val viewedContentDao: ViewedContentDao,
) : UserDataStore {

    override suspend fun addNewViewedContent(content: ViewedContent) {
        viewedContentDao.insert(content.toLocalData())
    }

    override suspend fun getRecentlyViewedContent(
        filteredTypes: List<ViewedContent.Type>?,
        limit: Int
    ): Flow<List<ViewedContent>> {
        val allowedTypes = filteredTypes ?: ViewedContent.Type.entries.toList()

        return viewedContentDao.getRecentlyViewedContent(allowedTypes.map { it.dbValue }, limit)
    }
}
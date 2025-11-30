package com.lelloman.pezzottify.android.localdata.internal.user

import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.localdata.internal.user.model.SearchHistoryEntryEntity
import com.lelloman.pezzottify.android.localdata.internal.user.model.dbValue
import com.lelloman.pezzottify.android.localdata.internal.user.model.toDomain
import com.lelloman.pezzottify.android.localdata.internal.user.model.toLocalData
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

internal class UserDataStoreImpl(
    private val viewedContentDao: ViewedContentDao,
    private val searchHistoryEntryDao: SearchHistoryEntryDao,
    private val timeProvider: () -> Long,
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

    override suspend fun addSearchHistoryEntry(
        query: String,
        contentType: SearchHistoryEntry.Type,
        contentId: String,
    ) {
        val entity = SearchHistoryEntryEntity(
            id = "${contentType.name}_$contentId",
            query = query,
            contentType = contentType,
            contentId = contentId,
            created = timeProvider(),
        )
        searchHistoryEntryDao.insert(entity)
    }

    override fun getSearchHistoryEntries(limit: Int): Flow<List<SearchHistoryEntry>> =
        searchHistoryEntryDao.getRecent(limit).map { entities ->
            entities.map { it.toDomain() }
        }

    override suspend fun deleteAll() {
        viewedContentDao.deleteAll()
        searchHistoryEntryDao.deleteAll()
    }
}
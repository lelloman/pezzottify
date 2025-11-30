package com.lelloman.pezzottify.android.domain.user

import kotlinx.coroutines.flow.Flow

interface UserDataStore {

    suspend fun addNewViewedContent(content: ViewedContent)

    suspend fun getRecentlyViewedContent(
        filteredTypes: List<ViewedContent.Type>?,
        limit: Int,
    ): Flow<List<ViewedContent>>

    suspend fun addSearchHistoryEntry(
        query: String,
        contentType: SearchHistoryEntry.Type,
        contentId: String,
    )

    fun getSearchHistoryEntries(limit: Int): Flow<List<SearchHistoryEntry>>

    suspend fun deleteAll()

}
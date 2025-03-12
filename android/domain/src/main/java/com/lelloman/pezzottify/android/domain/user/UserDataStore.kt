package com.lelloman.pezzottify.android.domain.user

import kotlinx.coroutines.flow.Flow

interface UserDataStore {

    suspend fun addNewViewedContent(content: ViewedContent)

    suspend fun getRecentlyViewedContent(
        filteredTypes: List<ViewedContent.Type>?,
        limit: Int,
    ): Flow<List<ViewedContent>>

}
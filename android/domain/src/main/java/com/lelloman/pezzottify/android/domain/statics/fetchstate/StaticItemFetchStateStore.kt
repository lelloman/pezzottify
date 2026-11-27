package com.lelloman.pezzottify.android.domain.statics.fetchstate

import kotlinx.coroutines.flow.Flow

interface StaticItemFetchStateStore {

    suspend fun store(record: StaticItemFetchState): Result<Unit>

    suspend fun resetLoadingStates(): Result<Unit>

    fun get(itemId: String): Flow<StaticItemFetchState?>

    suspend fun getIdle(): List<StaticItemFetchState>

    suspend fun getLoadingItemsCount(): Int

    /** Earliest scheduled retry time among non-loading items, or null if none is scheduled. */
    suspend fun getNextRetryTime(): Long?

    suspend fun delete(itemId: String): Result<Unit>
}
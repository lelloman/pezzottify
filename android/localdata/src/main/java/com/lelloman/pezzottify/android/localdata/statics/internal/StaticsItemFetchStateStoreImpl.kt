package com.lelloman.pezzottify.android.localdata.statics.internal

import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticItemFetchStateRecord.Companion.toDomain
import com.lelloman.pezzottify.android.localdata.statics.internal.StaticItemFetchStateRecord.Companion.toRecord
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

internal class StaticsItemFetchStateStoreImpl(
    private val dao: StaticItemFetchStateDao,
) : StaticItemFetchStateStore {

    override suspend fun store(record: StaticItemFetchState): Result<Unit> {
        val result = dao.insert(record.toRecord())
        return if (result != -1L) Result.success(Unit) else Result.failure(Exception("Could not store new record"))
    }

    override fun get(itemId: String): Flow<StaticItemFetchState?> {
        return dao.get(itemId).map {
            it?.toDomain()
        }
    }

    override suspend fun resetLoadingStates(): Result<Unit> {
        dao.resetLoadingStates()
        return Result.success(Unit)
    }

    override suspend fun getIdle(): List<StaticItemFetchState> =
        dao.getAllIdle().map { it.toDomain() }

    override suspend fun getLoadingItemsCount(): Int = dao.getLoadingItemsCount()

    override suspend fun delete(itemId: String): Result<Unit> {
        val result = dao.delete(itemId)
        return if (result == 1) Result.success(Unit) else Result.failure(Exception("Could not delete record"))
    }
}
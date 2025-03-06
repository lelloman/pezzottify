package com.lelloman.pezzottify.android.localdata.statics

import com.lelloman.pezzottify.android.localdata.statics.model.StaticItemFetchState
import kotlinx.coroutines.flow.Flow

interface StaticItemFetchStateStore {

    fun getFetchState(itemId: String): Flow<StaticItemFetchState>

    fun setFetchState(itemId: String, fetchState: StaticItemFetchState): Result<Void>
}
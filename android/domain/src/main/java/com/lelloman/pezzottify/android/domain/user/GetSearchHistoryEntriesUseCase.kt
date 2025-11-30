package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject

class GetSearchHistoryEntriesUseCase @Inject constructor(
    private val userDataStore: UserDataStore,
) : UseCase() {

    operator fun invoke(limit: Int): Flow<List<SearchHistoryEntry>> =
        userDataStore.getSearchHistoryEntries(limit)
}

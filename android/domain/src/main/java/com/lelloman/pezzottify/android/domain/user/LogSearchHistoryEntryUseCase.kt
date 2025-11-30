package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class LogSearchHistoryEntryUseCase @Inject constructor(
    private val userDataStore: UserDataStore,
) : UseCase() {

    operator fun invoke(query: String, contentType: SearchHistoryEntry.Type, contentId: String) {
        GlobalScope.launch(Dispatchers.IO) {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }
}

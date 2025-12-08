package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class LogSearchHistoryEntryUseCase @Inject constructor(
    private val userDataStore: UserDataStore,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    operator fun invoke(query: String, contentType: SearchHistoryEntry.Type, contentId: String) {
        logger.debug("invoke() logging search history: query='$query', type=$contentType, contentId=$contentId")
        GlobalScope.launch(Dispatchers.IO) {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }
}

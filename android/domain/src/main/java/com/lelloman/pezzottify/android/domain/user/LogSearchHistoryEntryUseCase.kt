package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject

class LogSearchHistoryEntryUseCase internal constructor(
    private val userDataStore: UserDataStore,
    private val scope: CoroutineScope,
    private val dispatcher: CoroutineDispatcher,
    private val logger: Logger,
) : UseCase() {

    @Inject
    constructor(
        userDataStore: UserDataStore,
        scope: CoroutineScope,
        loggerFactory: LoggerFactory,
    ) : this(
        userDataStore = userDataStore,
        scope = scope,
        dispatcher = Dispatchers.IO,
        logger = loggerFactory.getLogger(LogSearchHistoryEntryUseCase::class),
    )

    operator fun invoke(query: String, contentType: SearchHistoryEntry.Type, contentId: String) {
        logger.debug("invoke() logging search history: query='$query', type=$contentType, contentId=$contentId")
        scope.launch(dispatcher) {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }
}

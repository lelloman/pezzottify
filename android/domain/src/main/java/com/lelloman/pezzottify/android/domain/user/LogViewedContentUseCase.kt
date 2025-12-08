package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class LogViewedContentUseCase @Inject constructor(
    private val userDataStore: UserDataStore,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    operator fun invoke(contentId: String, type: ViewedContent.Type) {
        logger.debug("invoke() logging viewed content: $type $contentId")
        GlobalScope.launch(Dispatchers.IO) {
            userDataStore.addNewViewedContent(object : ViewedContent {
                override val type: ViewedContent.Type = type
                override val contentId: String = contentId
                override val created: Long = timeProvider.nowUtcMs()
                override val synced: Boolean = false
            })
        }
    }
}
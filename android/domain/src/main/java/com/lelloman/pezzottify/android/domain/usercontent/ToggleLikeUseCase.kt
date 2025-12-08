package com.lelloman.pezzottify.android.domain.usercontent

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class ToggleLikeUseCase @Inject constructor(
    private val userContentStore: UserContentStore,
    private val synchronizer: UserContentSynchronizer,
    private val timeProvider: TimeProvider,
    loggerFactory: LoggerFactory,
) {
    private val logger: Logger by loggerFactory

    operator fun invoke(
        contentId: String,
        type: LikedContent.ContentType,
        currentlyLiked: Boolean,
    ) {
        val newLikedState = !currentlyLiked
        logger.info("invoke() toggling like for $type $contentId: $currentlyLiked -> $newLikedState")
        GlobalScope.launch(Dispatchers.IO) {
            userContentStore.setLiked(
                contentId = contentId,
                type = type,
                liked = newLikedState,
                modifiedAt = timeProvider.nowUtcMs(),
            )
            logger.debug("invoke() like state saved, waking up synchronizer")
            synchronizer.wakeUp()
        }
    }
}

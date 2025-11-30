package com.lelloman.pezzottify.android.domain.usercontent

import com.lelloman.pezzottify.android.domain.app.TimeProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import javax.inject.Inject

class ToggleLikeUseCase @Inject constructor(
    private val userContentStore: UserContentStore,
    private val synchronizer: UserContentSynchronizer,
    private val timeProvider: TimeProvider,
) {
    operator fun invoke(
        contentId: String,
        type: LikedContent.ContentType,
        currentlyLiked: Boolean,
    ) {
        GlobalScope.launch(Dispatchers.IO) {
            userContentStore.setLiked(
                contentId = contentId,
                type = type,
                liked = !currentlyLiked,
                modifiedAt = timeProvider.nowUtcMs(),
            )
            synchronizer.wakeUp()
        }
    }
}

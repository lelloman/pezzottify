package com.lelloman.pezzottify.android.domain.usercontent

import kotlinx.coroutines.flow.Flow
import javax.inject.Inject

class GetLikedStateUseCase @Inject constructor(
    private val userContentStore: UserContentStore,
) {
    operator fun invoke(contentId: String): Flow<Boolean> = userContentStore.isLiked(contentId)
}

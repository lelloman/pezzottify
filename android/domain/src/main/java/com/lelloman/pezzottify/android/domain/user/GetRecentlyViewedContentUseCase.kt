package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject


class GetRecentlyViewedContentUseCase @Inject constructor(private val userDataStore: UserDataStore) :
    UseCase() {

    suspend operator fun invoke(limit: Int): Flow<List<ViewedContent>> = userDataStore
        .getRecentlyViewedContent(
            listOf(ViewedContent.Type.Artist, ViewedContent.Type.Album),
            limit
        )
}
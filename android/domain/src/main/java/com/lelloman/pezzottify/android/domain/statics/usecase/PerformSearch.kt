package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import javax.inject.Inject


class PerformSearch @Inject constructor(
    private val remoteApiClient: RemoteApiClient
) : UseCase() {

    suspend operator fun invoke(query: String): Result<List<Pair<String, SearchedItemType>>> {
        return when (val response = remoteApiClient.search(query, null)) {
            is RemoteApiResponse.Success -> Result.success(response.data.map { it.itemId to it.itemType })
            is RemoteApiResponse.Error -> Result.failure(Throwable())
        }
    }
}
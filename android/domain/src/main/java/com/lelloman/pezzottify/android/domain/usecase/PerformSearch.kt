package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import javax.inject.Inject


class PerformSearch @Inject constructor(
    private val remoteApiClient: RemoteApiClient
) : UseCase() {

    suspend operator fun invoke(query: String): Result<List<String>> {
        return when (val response = remoteApiClient.search(query, null)) {
            is RemoteApiResponse.Success -> Result.success(response.data.map { it.itemId })
            is RemoteApiResponse.Error -> Result.failure(Throwable())
        }
    }
}
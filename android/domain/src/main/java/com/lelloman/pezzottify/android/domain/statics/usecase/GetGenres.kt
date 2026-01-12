package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.GenreResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import javax.inject.Inject

/**
 * Use case to fetch all available genres with track counts.
 */
class GetGenres @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(limit: Int = 100): Result<List<GenreResponse>> {
        return when (val response = remoteApiClient.getGenres()) {
            is RemoteApiResponse.Success -> {
                val limitedGenres = response.data.take(limit)
                Result.success(limitedGenres)
            }
            is RemoteApiResponse.Error -> Result.failure(
                Exception("Failed to fetch genres: ${response::class.simpleName}")
            )
        }
    }
}

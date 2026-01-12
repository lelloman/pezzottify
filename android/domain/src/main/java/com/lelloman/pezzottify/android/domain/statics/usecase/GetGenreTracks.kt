package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.GenreTracksResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import javax.inject.Inject

/**
 * Use case to fetch tracks for a specific genre.
 */
class GetGenreTracks @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(
        genreName: String,
        limit: Int = 50,
        offset: Int = 0,
    ): Result<GenreTracksResponse> {
        return when (val response = remoteApiClient.getGenreTracks(genreName, limit, offset)) {
            is RemoteApiResponse.Success -> Result.success(response.data)
            is RemoteApiResponse.Error -> Result.failure(
                Exception("Failed to fetch genre tracks: ${response::class.simpleName}")
            )
        }
    }
}

package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.ExternalAlbumDetailsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Fetches detailed information about an external album.
 * Includes track listing, cover image, and download request status.
 */
class GetExternalAlbumDetailsUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Get detailed information about an external album.
     *
     * @param albumId External album ID from the music provider
     * @return Result containing album details or failure
     */
    suspend operator fun invoke(albumId: String): Result<ExternalAlbumDetailsResponse> {
        logger.info("invoke() fetching external album details: $albumId")
        return when (val response = remoteApiClient.getExternalAlbumDetails(albumId)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() got external album: ${response.data.name} with ${response.data.tracks.size} tracks")
                Result.success(response.data)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() failed to get external album details: $response")
                Result.failure(GetExternalAlbumDetailsException(response.toString()))
            }
        }
    }
}

class GetExternalAlbumDetailsException(message: String) : Exception(message)

package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RequestAlbumResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Requests download of an album from the external downloader service.
 */
class RequestAlbumDownloadUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Request download of an album.
     *
     * @param albumId External album ID from the music provider
     * @param albumName Album name for display
     * @param artistName Artist name for display
     * @return Result containing request result or failure
     */
    suspend operator fun invoke(
        albumId: String,
        albumName: String,
        artistName: String
    ): Result<RequestAlbumResponse> {
        logger.info("invoke() requesting album download: $albumName by $artistName (id: $albumId)")
        return when (val response = remoteApiClient.requestAlbumDownload(albumId, albumName, artistName)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() album download requested: requestId=${response.data.requestId}, queuePosition=${response.data.queuePosition}")
                Result.success(response.data)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() request album download failed: $response")
                Result.failure(RequestAlbumDownloadException(response.toString()))
            }
        }
    }
}

class RequestAlbumDownloadException(message: String) : Exception(message)

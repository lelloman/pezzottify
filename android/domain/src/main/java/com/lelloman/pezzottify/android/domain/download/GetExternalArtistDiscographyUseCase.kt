package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.ExternalDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Fetches an external artist's discography with download status.
 * Returns album list with in_catalog, in_queue, and request_status fields.
 */
class GetExternalArtistDiscographyUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Get an external artist's discography.
     *
     * @param artistId External artist ID from the music provider
     * @return Result containing artist info and albums or failure
     */
    suspend operator fun invoke(artistId: String): Result<ExternalDiscographyResponse> {
        logger.info("invoke() fetching external discography for artist: $artistId")
        return when (val response = remoteApiClient.getExternalDiscography(artistId)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() got discography: ${response.data.artist.name} with ${response.data.albums.size} albums")
                Result.success(response.data)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() failed to get external discography: $response")
                Result.failure(GetExternalDiscographyException(response.toString()))
            }
        }
    }
}

class GetExternalDiscographyException(message: String) : Exception(message)

package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.MyDownloadRequestsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Gets the user's download requests from the download manager.
 */
class GetMyDownloadRequestsUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Get user's download requests.
     *
     * @param limit Maximum number of requests to return (optional)
     * @param offset Offset for pagination (optional)
     * @return Result containing download requests and limits, or failure
     */
    suspend operator fun invoke(
        limit: Int? = null,
        offset: Int? = null
    ): Result<MyDownloadRequestsResponse> {
        logger.info("invoke() getting my download requests (limit=$limit, offset=$offset)")
        return when (val response = remoteApiClient.getMyDownloadRequests(limit, offset)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() got ${response.data.requests.size} download requests")
                Result.success(response.data)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() get my download requests failed: $response")
                Result.failure(GetMyDownloadRequestsException(response.toString()))
            }
        }
    }
}

class GetMyDownloadRequestsException(message: String) : Exception(message)

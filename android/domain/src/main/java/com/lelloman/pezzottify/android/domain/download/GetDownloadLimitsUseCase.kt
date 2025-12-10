package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadLimitsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Gets the user's rate limit status for download requests.
 */
class GetDownloadLimitsUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Get user's current rate limit status for download requests.
     *
     * @return Result containing download limits or failure
     */
    suspend operator fun invoke(): Result<DownloadLimitsResponse> {
        logger.info("invoke() getting download limits")
        return when (val response = remoteApiClient.getDownloadLimits()) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() download limits: today=${response.data.requestsToday}/${response.data.maxPerDay}, queue=${response.data.inQueue}/${response.data.maxQueue}")
                Result.success(response.data)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() get download limits failed: $response")
                Result.failure(DownloadLimitsException(response.toString()))
            }
        }
    }
}

class DownloadLimitsException(message: String) : Exception(message)

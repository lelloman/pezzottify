package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.ExternalSearchResult
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject

/**
 * Performs external search via the download manager API.
 * Searches for albums or artists from external music providers.
 */
class PerformExternalSearchUseCase @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    /**
     * Search for content in the external downloader service.
     *
     * @param query The search query
     * @param type The type of content to search for (Album or Artist)
     * @return Result containing list of search results or failure
     */
    suspend operator fun invoke(
        query: String,
        type: RemoteApiClient.ExternalSearchType
    ): Result<List<ExternalSearchResult>> {
        logger.info("invoke() searching externally for: $query, type: $type")
        return when (val response = remoteApiClient.externalSearch(query, type)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() external search returned ${response.data.results.size} results")
                Result.success(response.data.results)
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() external search failed: $response")
                Result.failure(ExternalSearchException(response.toString()))
            }
        }
    }
}

class ExternalSearchException(message: String) : Exception(message)

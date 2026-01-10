package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject


class PerformSearch @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    suspend operator fun invoke(
        query: String,
        filters: List<RemoteApiClient.SearchFilter>? = null,
        excludeUnavailable: Boolean = false
    ): Result<List<Pair<String, SearchedItemType>>> {
        logger.info("invoke() searching for: $query, filters: $filters, excludeUnavailable: $excludeUnavailable")
        return when (val response = remoteApiClient.search(query, filters, excludeUnavailable)) {
            is RemoteApiResponse.Success -> {
                logger.debug("invoke() search returned ${response.data.size} results")
                Result.success(response.data.map { it.itemId to it.itemType })
            }
            is RemoteApiResponse.Error -> {
                logger.error("invoke() search failed: $response")
                Result.failure(Throwable())
            }
        }
    }
}
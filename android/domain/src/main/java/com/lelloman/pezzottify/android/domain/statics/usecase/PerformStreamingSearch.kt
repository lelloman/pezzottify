package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.onCompletion
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.onStart
import javax.inject.Inject

/**
 * Use case for performing streaming search with SSE.
 * Returns a Flow of SearchSection objects as they arrive from the server.
 */
class PerformStreamingSearch @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : UseCase() {

    private val logger: Logger by loggerFactory

    operator fun invoke(query: String): Flow<SearchSection> {
        logger.info("invoke() streaming search for: $query")
        return remoteApiClient.streamingSearch(query)
            .onStart {
                logger.debug("invoke() streaming search started")
            }
            .onEach { section ->
                logger.debug("invoke() received section: ${section::class.simpleName}")
            }
            .onCompletion { error ->
                if (error != null) {
                    logger.error("invoke() streaming search failed: $error")
                } else {
                    logger.debug("invoke() streaming search completed")
                }
            }
            .catch { e ->
                logger.error("invoke() streaming search error: $e")
                throw e
            }
    }
}

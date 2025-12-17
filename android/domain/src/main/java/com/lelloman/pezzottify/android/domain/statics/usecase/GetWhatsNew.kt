package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.WhatsNewBatch
import com.lelloman.pezzottify.android.domain.remoteapi.response.WhatsNewResponse
import javax.inject.Inject

/**
 * Use case to fetch recent catalog updates ("What's New").
 */
class GetWhatsNew @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(limit: Int = 10): Result<WhatsNewResponse> {
        return when (val response = remoteApiClient.getWhatsNew(limit)) {
            is RemoteApiResponse.Success -> Result.success(response.data)
            is RemoteApiResponse.Error -> Result.failure(
                Exception("Failed to fetch what's new: ${response::class.simpleName}")
            )
        }
    }
}

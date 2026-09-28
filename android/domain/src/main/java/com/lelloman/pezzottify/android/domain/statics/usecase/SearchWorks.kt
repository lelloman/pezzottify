package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.Work
import javax.inject.Inject

class SearchWorks @Inject constructor(private val api: RemoteApiClient) {
    suspend operator fun invoke(query: String): List<Work> = when (val response = api.searchWorks(query)) {
        is RemoteApiResponse.Success -> response.data
        is RemoteApiResponse.Error -> error("Could not search works")
    }
}

package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.WorkPage
import com.lelloman.pezzottify.android.domain.statics.WorkScope
import javax.inject.Inject

class WorkNotFoundException : Exception()

class GetWork @Inject constructor(private val api: RemoteApiClient) {
    suspend operator fun invoke(id: String, offset: Int = 0, scope: WorkScope = WorkScope.All, limit: Int = 50): WorkPage =
        when (val response = api.getWork(id, limit, offset, scope.value)) {
            is RemoteApiResponse.Success -> response.data.also {
                check(!it.hasMore || it.nextOffset > offset) { "Work pagination did not advance" }
            }
            RemoteApiResponse.Error.NotFound -> throw WorkNotFoundException()
            is RemoteApiResponse.Error -> error("Could not load work")
        }
}

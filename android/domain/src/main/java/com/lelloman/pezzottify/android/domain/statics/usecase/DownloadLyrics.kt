package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import javax.inject.Inject

class DownloadLyrics @Inject constructor(private val api: RemoteApiClient) {
    suspend operator fun invoke(entityType: String, id: String) = api.downloadLyrics(entityType, id)
}

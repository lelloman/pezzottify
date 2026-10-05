package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import javax.inject.Inject

class GetTrackLyrics @Inject constructor(private val api: RemoteApiClient) {
    suspend operator fun invoke(trackId: String) = api.getTrackLyrics(trackId)
}

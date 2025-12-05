package com.lelloman.pezzottify.android.domain.remoteapi

import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventRecordedResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable


data class DeviceInfo(
    val deviceUuid: String,
    val deviceType: String,
    val deviceName: String?,
    val osInfo: String?,
)

interface RemoteApiClient {

    suspend fun login(
        userHandle: String,
        password: String,
        deviceInfo: DeviceInfo,
    ): RemoteApiResponse<LoginSuccessResponse>

    suspend fun logout(): RemoteApiResponse<Unit>

    suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse>

    suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse>

    suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse>

    suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse>

    suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse>

    suspend fun search(
        query: String,
        filters: List<SearchFilter>? = null
    ): RemoteApiResponse<SearchResponse>

    suspend fun getLikedContent(contentType: String): RemoteApiResponse<List<String>>

    suspend fun likeContent(contentType: String, contentId: String): RemoteApiResponse<Unit>

    suspend fun unlikeContent(contentType: String, contentId: String): RemoteApiResponse<Unit>

    suspend fun recordListeningEvent(data: ListeningEventSyncData): RemoteApiResponse<ListeningEventRecordedResponse>

    /**
     * Get full user sync state for initial sync.
     */
    suspend fun getSyncState(): RemoteApiResponse<SyncStateResponse>

    /**
     * Get sync events since a given sequence number.
     * Returns RemoteApiResponse.Error.EventsPruned if the sequence has been pruned.
     */
    suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResponse>

    @Serializable
    enum class SearchFilter {
        Album,
        Artist,
        Track,
    }

    interface HostUrlProvider {
        val hostUrl: StateFlow<String>
    }
}
package com.lelloman.pezzottify.android.remoteapi

import com.lelloman.pezzottify.android.remoteapi.internal.RemoteApiClientImpl
import com.lelloman.pezzottify.android.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.remoteapi.response.TrackResponse
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import okhttp3.OkHttpClient


interface RemoteApiClient {

    suspend fun login(userHandle: String, password: String): RemoteApiResponse<LoginSuccessResponse>

    suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse>

    suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse>

    suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse>

    suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse>

    suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse>

    suspend fun search(
        query: String,
        filters: List<SearchFilter>? = null
    ): RemoteApiResponse<SearchResponse>

    object Factory {
        fun create(
            hostUrlProvider: HostUrlProvider,
            credentialsProvider: RemoteApiCredentialsProvider
        ): RemoteApiClient = RemoteApiClientImpl(
            hostUrlProvider = hostUrlProvider,
            okhttpClientBuilder = OkHttpClient.Builder(),
            credentialsProvider = credentialsProvider,
        )
    }

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
package com.lelloman.pezzottify.android.remoteapi

import com.lelloman.pezzottify.android.remoteapi.internal.RemoteApiClientImpl
import com.lelloman.pezzottify.android.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.remoteapi.response.ArtistDiscography
import com.lelloman.pezzottify.android.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.remoteapi.response.TrackResponse
import okhttp3.OkHttpClient


interface RemoteApiClient {

    suspend fun login(userHandle: String, password: String): RemoteApiResponse<LoginSuccessResponse>

    suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse>

    suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<List<ArtistDiscography>>

    suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse>

    suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse>

    suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse>

    suspend fun search(
        query: String,
        filters: List<SearchFilter>? = null
    ): RemoteApiResponse<List<String>>

    object Factory {
        fun create(
            baseUrl: String,
            credentialsProvider: RemoteApiCredentialsProvider
        ): RemoteApiClient = RemoteApiClientImpl(
            baseUrl = baseUrl,
            okhttpClientBuilder = OkHttpClient.Builder(),
            credentialsProvider = credentialsProvider,
        )
    }

    enum class SearchFilter {
        Album,
        Artist,
        Track,
    }
}
package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
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
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import retrofit2.Response
import retrofit2.Retrofit
import retrofit2.converter.kotlinx.serialization.asConverterFactory

internal class RemoteApiClientImpl(
    hostUrlProvider: RemoteApiClient.HostUrlProvider,
    private val okhttpClientBuilder: OkHttpClient.Builder,
    private val credentialsProvider: RemoteApiCredentialsProvider,
    coroutineScope: CoroutineScope = GlobalScope,
) : RemoteApiClient {

    private val authToken get() = credentialsProvider.authToken

    @OptIn(ExperimentalSerializationApi::class)
    private val jsonConverter = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
    }

    private val retrofitFlow = hostUrlProvider.hostUrl.map(::makeRetrofit)
        .stateIn(coroutineScope, SharingStarted.Eagerly, null)

    private val <T>Response<T>.commonError
        get() = if (!isSuccessful) {
            when (code()) {
                403 -> RemoteApiResponse.Error.Unauthorized
                404 -> RemoteApiResponse.Error.NotFound
                else -> RemoteApiResponse.Error.Unknown(message())
            }
        } else {
            null
        }

    private val noUrlRetrofit by lazy { makeRetrofit("http://localhost") }
    private val retrofit get() = retrofitFlow.value ?: noUrlRetrofit

    private val <T>Response<T>.parsedBody: RemoteApiResponse<T>
        get() = try {
            RemoteApiResponse.Success(body()!!)
        } catch (t: Throwable) {
            RemoteApiResponse.Error.Unknown(t.message ?: "Unknown error")
        }

    private fun <T> Response<T>.returnFromRetrofitResponse(): RemoteApiResponse<T> =
        commonError ?: parsedBody

    private fun makeRetrofit(baseUrl: String) = Retrofit.Builder()
        .client(okhttpClientBuilder.build())
        .baseUrl(baseUrl)
        .addConverterFactory(jsonConverter.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(RetrofitApiClient::class.java)

    override suspend fun login(
        userHandle: String,
        password: String,
        deviceInfo: DeviceInfo,
    ): RemoteApiResponse<LoginSuccessResponse> = catchingNetworkError {
        retrofit
            .login(
                LoginRequest(
                    userHandle = userHandle,
                    password = password,
                    deviceUuid = deviceInfo.deviceUuid,
                    deviceType = deviceInfo.deviceType,
                    deviceName = deviceInfo.deviceName,
                    osInfo = deviceInfo.osInfo,
                )
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun logout(): RemoteApiResponse<Unit> = catchingNetworkError {
        retrofit
            .logout(authToken = authToken)
            .returnFromRetrofitResponse()
    }

    override suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse> =
        catchingNetworkError {
            retrofit
                .getArtist(authToken = authToken, artistId = artistId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse> =
        catchingNetworkError {
            retrofit
                .getArtistDiscography(authToken = authToken, artistId = artistId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse> =
        catchingNetworkError {
            retrofit
                .getAlbum(authToken = authToken, albumId = albumId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse> =
        catchingNetworkError {
            retrofit
                .getTrack(authToken = authToken, trackId = trackId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse> =
        catchingNetworkError {
            val retrofitResponse = retrofit.getImage(authToken = authToken, imageId = imageId)
            retrofitResponse.commonError?.let { return@catchingNetworkError it }
            val imageBytes = retrofitResponse.body()?.bytes()
                ?: return@catchingNetworkError RemoteApiResponse.Error.Unknown("No body")

            val mimeType = retrofitResponse.headers()["Content-Type"] ?: "image/*"

            return@catchingNetworkError RemoteApiResponse.Success(
                ImageResponse(
                    mimeType = mimeType,
                    content = imageBytes,
                )
            )
        }

    override suspend fun search(
        query: String,
        filters: List<RemoteApiClient.SearchFilter>?
    ): RemoteApiResponse<SearchResponse> = catchingNetworkError {
        retrofit
            .search(authToken, SearchRequest(query, filters?.map { it.name.lowercase() }))
            .returnFromRetrofitResponse()
    }

    override suspend fun getLikedContent(contentType: String): RemoteApiResponse<List<String>> =
        catchingNetworkError {
            retrofit
                .getLikedContent(authToken = authToken, contentType = contentType)
                .returnFromRetrofitResponse()
        }

    override suspend fun likeContent(contentType: String, contentId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            retrofit
                .likeContent(authToken = authToken, contentType = contentType, contentId = contentId)
                .returnFromRetrofitResponse()
        }

    override suspend fun unlikeContent(contentType: String, contentId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            retrofit
                .unlikeContent(authToken = authToken, contentType = contentType, contentId = contentId)
                .returnFromRetrofitResponse()
        }

    override suspend fun recordListeningEvent(
        data: ListeningEventSyncData
    ): RemoteApiResponse<ListeningEventRecordedResponse> = catchingNetworkError {
        val request = ListeningEventRequest(
            trackId = data.trackId,
            sessionId = data.sessionId,
            startedAt = data.startedAt,
            endedAt = data.endedAt,
            durationSeconds = data.durationSeconds,
            trackDurationSeconds = data.trackDurationSeconds,
            seekCount = data.seekCount,
            pauseCount = data.pauseCount,
            playbackContext = data.playbackContext,
            clientType = "android",
        )
        val response = retrofit.recordListeningEvent(authToken = authToken, request = request)
        response.commonError?.let { return@catchingNetworkError it }
        val body = response.body()
            ?: return@catchingNetworkError RemoteApiResponse.Error.Unknown("No body")
        RemoteApiResponse.Success(
            ListeningEventRecordedResponse(id = body.id, created = body.created)
        )
    }

    override suspend fun getSyncState(): RemoteApiResponse<SyncStateResponse> =
        catchingNetworkError {
            retrofit
                .getSyncState(authToken = authToken)
                .returnFromRetrofitResponse()
        }

    override suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResponse> =
        catchingNetworkError {
            val response = retrofit.getSyncEvents(authToken = authToken, since = since)
            // Handle 410 Gone (events pruned)
            if (response.code() == 410) {
                return@catchingNetworkError RemoteApiResponse.Error.EventsPruned
            }
            response.returnFromRetrofitResponse()
        }

    private suspend fun <T> catchingNetworkError(block: suspend () -> RemoteApiResponse<T>): RemoteApiResponse<T> =
        try {
            block()
        } catch (t: Throwable) {
            RemoteApiResponse.Error.Network
        }
}
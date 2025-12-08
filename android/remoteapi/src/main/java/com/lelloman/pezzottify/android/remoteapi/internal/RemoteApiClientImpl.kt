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
import com.lelloman.pezzottify.android.domain.remoteapi.response.PopularContentResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.UpdateUserSettingsRequest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import okhttp3.HttpUrl.Companion.toHttpUrl
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

    private fun isValidHttpUrl(url: String): Boolean {
        if (url.isBlank()) return false
        return try {
            val httpUrl = url.toHttpUrl()
            httpUrl.scheme == "http" || httpUrl.scheme == "https"
        } catch (_: IllegalArgumentException) {
            false
        }
    }

    @OptIn(ExperimentalSerializationApi::class)
    private val jsonConverter = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
    }

    private val retrofitFlow = hostUrlProvider.hostUrl
        .map { url -> url.takeIf { isValidHttpUrl(it) } }
        .map { validUrl -> validUrl?.let(::makeRetrofit) }
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

    private suspend fun getRetrofit(): RetrofitApiClient =
        retrofitFlow.filterNotNull().first()

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
        getRetrofit()
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
        getRetrofit()
            .logout(authToken = authToken)
            .returnFromRetrofitResponse()
    }

    override suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse> =
        catchingNetworkError {
            getRetrofit()
                .getArtist(authToken = authToken, artistId = artistId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse> =
        catchingNetworkError {
            getRetrofit()
                .getArtistDiscography(authToken = authToken, artistId = artistId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse> =
        catchingNetworkError {
            getRetrofit()
                .getAlbum(authToken = authToken, albumId = albumId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse> =
        catchingNetworkError {
            getRetrofit()
                .getTrack(authToken = authToken, trackId = trackId)
                .returnFromRetrofitResponse()
        }

    override suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse> =
        catchingNetworkError {
            val retrofitResponse = getRetrofit().getImage(authToken = authToken, imageId = imageId)
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

    override suspend fun getPopularContent(
        albumsLimit: Int,
        artistsLimit: Int,
    ): RemoteApiResponse<PopularContentResponse> = catchingNetworkError {
        getRetrofit()
            .getPopularContent(
                authToken = authToken,
                albumsLimit = albumsLimit,
                artistsLimit = artistsLimit,
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun search(
        query: String,
        filters: List<RemoteApiClient.SearchFilter>?
    ): RemoteApiResponse<SearchResponse> = catchingNetworkError {
        getRetrofit()
            .search(authToken, SearchRequest(query, filters?.map { it.name.lowercase() }))
            .returnFromRetrofitResponse()
    }

    override suspend fun getLikedContent(contentType: String): RemoteApiResponse<List<String>> =
        catchingNetworkError {
            getRetrofit()
                .getLikedContent(authToken = authToken, contentType = contentType)
                .returnFromRetrofitResponse()
        }

    override suspend fun likeContent(contentType: String, contentId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
                .likeContent(authToken = authToken, contentType = contentType, contentId = contentId)
                .returnFromRetrofitResponse()
        }

    override suspend fun unlikeContent(contentType: String, contentId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
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
        val response = getRetrofit().recordListeningEvent(authToken = authToken, request = request)
        response.commonError?.let { return@catchingNetworkError it }
        val body = response.body()
            ?: return@catchingNetworkError RemoteApiResponse.Error.Unknown("No body")
        RemoteApiResponse.Success(
            ListeningEventRecordedResponse(id = body.id, created = body.created)
        )
    }

    override suspend fun getSyncState(): RemoteApiResponse<SyncStateResponse> =
        catchingNetworkError {
            getRetrofit()
                .getSyncState(authToken = authToken)
                .returnFromRetrofitResponse()
        }

    override suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResponse> =
        catchingNetworkError {
            val response = getRetrofit().getSyncEvents(authToken = authToken, since = since)
            // Handle 410 Gone (events pruned)
            if (response.code() == 410) {
                return@catchingNetworkError RemoteApiResponse.Error.EventsPruned
            }
            response.returnFromRetrofitResponse()
        }

    override suspend fun updateUserSettings(settings: List<UserSetting>): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
                .updateUserSettings(
                    authToken = authToken,
                    request = UpdateUserSettingsRequest(settings = settings)
                )
                .returnFromRetrofitResponse()
        }

    private suspend fun <T> catchingNetworkError(block: suspend () -> RemoteApiResponse<T>): RemoteApiResponse<T> =
        try {
            block()
        } catch (t: Throwable) {
            RemoteApiResponse.Error.Network
        }
}
package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadLimitsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.FullSkeletonResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventItem
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventRecordedResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.MyDownloadRequestsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.PopularContentResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RequestAlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonDeltaResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonVersionResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.WhatsNewResponse
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.remoteapi.SubmitBugReportResponse
import com.lelloman.pezzottify.android.remoteapi.internal.requests.AddTracksToPlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.CreatePlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ImpressionRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.RemoveTracksFromPlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.RequestAlbumDownloadBody
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SubmitBugReportRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.UpdatePlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.UpdateUserSettingsRequest
import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.URLEncoder
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import retrofit2.Response
import retrofit2.Retrofit
import retrofit2.converter.kotlinx.serialization.asConverterFactory

internal class RemoteApiClientImpl(
    private val hostUrlProvider: RemoteApiClient.HostUrlProvider,
    private val okHttpClientFactory: OkHttpClientFactory,
    private val credentialsProvider: RemoteApiCredentialsProvider,
    coroutineScope: CoroutineScope = GlobalScope,
    private val interceptors: List<Interceptor> = emptyList(),
    private val logger: Logger? = null,
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
        classDiscriminator = "section"
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

    private fun <T> Response<T>.parsedBody(): RemoteApiResponse<T> =
        try {
            val body = body()
            if (body == null) {
                @Suppress("UNCHECKED_CAST")
                RemoteApiResponse.Success(Unit as T)
            } else {
                RemoteApiResponse.Success(body)
            }
        } catch (t: Throwable) {
            RemoteApiResponse.Error.Unknown(t.message ?: "Unknown error")
        }

    private fun <T> Response<T>.returnFromRetrofitResponse(): RemoteApiResponse<T> =
        commonError ?: parsedBody()

    private fun makeRetrofit(baseUrl: String): RetrofitApiClient {
        val okHttpBuilder = okHttpClientFactory.createBuilder(baseUrl)
        interceptors.forEach { okHttpBuilder.addInterceptor(it) }
        return Retrofit.Builder()
            .client(okHttpBuilder.build())
            .baseUrl(baseUrl)
            .addConverterFactory(jsonConverter.asConverterFactory("application/json".toMediaType()))
            .build()
            .create(RetrofitApiClient::class.java)
    }

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

    override suspend fun getArtistDiscography(
        artistId: String,
        offset: Int?,
        limit: Int?
    ): RemoteApiResponse<ArtistDiscographyResponse> =
        catchingNetworkError {
            getRetrofit()
                .getArtistDiscography(authToken = authToken, artistId = artistId, offset = offset, limit = limit)
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

    override suspend fun getWhatsNew(limit: Int): RemoteApiResponse<WhatsNewResponse> =
        catchingNetworkError {
            getRetrofit()
                .getWhatsNew(authToken = authToken, limit = limit)
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

    override suspend fun recordImpression(itemType: String, itemId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
                .recordImpression(
                    authToken = authToken,
                    request = ImpressionRequest(itemType = itemType, itemId = itemId)
                )
                .returnFromRetrofitResponse()
        }

    override suspend fun getListeningEvents(
        startDate: Int?,
        endDate: Int?,
        limit: Int?,
        offset: Int?,
    ): RemoteApiResponse<List<ListeningEventItem>> = catchingNetworkError {
        getRetrofit()
            .getListeningEvents(
                authToken = authToken,
                startDate = startDate,
                endDate = endDate,
                limit = limit,
                offset = offset,
            )
            .returnFromRetrofitResponse()
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

    // Download manager endpoints

    override suspend fun getDownloadLimits(): RemoteApiResponse<DownloadLimitsResponse> =
        catchingNetworkError {
            getRetrofit()
                .getDownloadLimits(authToken = authToken)
                .returnFromRetrofitResponse()
        }

    override suspend fun requestAlbumDownload(
        albumId: String,
        albumName: String,
        artistName: String
    ): RemoteApiResponse<RequestAlbumResponse> = catchingNetworkError {
        getRetrofit()
            .requestAlbumDownload(
                authToken = authToken,
                request = RequestAlbumDownloadBody(
                    albumId = albumId,
                    albumName = albumName,
                    artistName = artistName
                )
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun getMyDownloadRequests(
        limit: Int?,
        offset: Int?
    ): RemoteApiResponse<MyDownloadRequestsResponse> = catchingNetworkError {
        getRetrofit()
            .getMyDownloadRequests(
                authToken = authToken,
                limit = limit,
                offset = offset
            )
            .returnFromRetrofitResponse()
    }

    // Skeleton sync endpoints

    override suspend fun getSkeletonVersion(): RemoteApiResponse<SkeletonVersionResponse> =
        catchingNetworkError {
            getRetrofit()
                .getSkeletonVersion(authToken = authToken)
                .returnFromRetrofitResponse()
        }

    override suspend fun getFullSkeleton(): RemoteApiResponse<FullSkeletonResponse> =
        catchingNetworkError {
            getRetrofit()
                .getFullSkeleton(authToken = authToken)
                .returnFromRetrofitResponse()
        }

    override suspend fun getSkeletonDelta(sinceVersion: Long): RemoteApiResponse<SkeletonDeltaResponse> =
        catchingNetworkError {
            val response = getRetrofit().getSkeletonDelta(authToken = authToken, sinceVersion = sinceVersion)
            // Handle 404 (version too old/pruned)
            if (response.code() == 404) {
                return@catchingNetworkError RemoteApiResponse.Error.NotFound
            }
            response.returnFromRetrofitResponse()
        }

    override suspend fun markNotificationRead(notificationId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
                .markNotificationRead(authToken = authToken, notificationId = notificationId)
                .returnFromRetrofitResponse()
        }

    // Playlist endpoints

    override suspend fun createPlaylist(
        name: String,
        trackIds: List<String>,
    ): RemoteApiResponse<String> = catchingNetworkError {
        val response = getRetrofit().createPlaylist(
            authToken = authToken,
            request = CreatePlaylistRequest(name = name, trackIds = trackIds)
        )
        response.commonError?.let { return@catchingNetworkError it }
        val body = response.body()
            ?: return@catchingNetworkError RemoteApiResponse.Error.Unknown("No body")
        RemoteApiResponse.Success(body.id)
    }

    override suspend fun updatePlaylist(
        playlistId: String,
        name: String?,
        trackIds: List<String>?,
    ): RemoteApiResponse<Unit> = catchingNetworkError {
        getRetrofit()
            .updatePlaylist(
                authToken = authToken,
                playlistId = playlistId,
                request = UpdatePlaylistRequest(name = name, trackIds = trackIds)
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun deletePlaylist(playlistId: String): RemoteApiResponse<Unit> =
        catchingNetworkError {
            getRetrofit()
                .deletePlaylist(authToken = authToken, playlistId = playlistId)
                .returnFromRetrofitResponse()
        }

    override suspend fun addTracksToPlaylist(
        playlistId: String,
        trackIds: List<String>,
    ): RemoteApiResponse<Unit> = catchingNetworkError {
        getRetrofit()
            .addTracksToPlaylist(
                authToken = authToken,
                playlistId = playlistId,
                request = AddTracksToPlaylistRequest(tracksIds = trackIds)
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun removeTracksFromPlaylist(
        playlistId: String,
        positions: List<Int>,
    ): RemoteApiResponse<Unit> = catchingNetworkError {
        getRetrofit()
            .removeTracksFromPlaylist(
                authToken = authToken,
                playlistId = playlistId,
                request = RemoveTracksFromPlaylistRequest(tracksPositions = positions)
            )
            .returnFromRetrofitResponse()
    }

    override suspend fun submitBugReport(
        title: String?,
        description: String,
        clientVersion: String?,
        deviceInfo: String?,
        logs: String?,
        attachments: List<String>?,
    ): RemoteApiResponse<SubmitBugReportResponse> = catchingNetworkError {
        getRetrofit()
            .submitBugReport(
                authToken = authToken,
                request = SubmitBugReportRequest(
                    title = title,
                    description = description,
                    clientType = "android",
                    clientVersion = clientVersion,
                    deviceInfo = deviceInfo,
                    logs = logs,
                    attachments = attachments,
                )
            )
            .returnFromRetrofitResponse()
    }

    // Streaming search (SSE)

    override fun streamingSearch(query: String): Flow<SearchSection> = flow {
        val baseUrl = hostUrlProvider.hostUrl.first { isValidHttpUrl(it) }
        val encodedQuery = URLEncoder.encode(query, "UTF-8")
        val url = "$baseUrl/v1/content/search/stream?q=$encodedQuery"

        val request = Request.Builder()
            .url(url)
            .header("Authorization", authToken)
            .header("Accept", "text/event-stream")
            .get()
            .build()

        val client = okHttpClientFactory.createBuilder(baseUrl).build()
        val response = client.newCall(request).execute()

        if (!response.isSuccessful) {
            throw Exception("SSE request failed: ${response.code}")
        }

        val body = response.body ?: throw Exception("No response body")
        val reader = BufferedReader(InputStreamReader(body.byteStream()))

        try {
            var line: String?
            while (reader.readLine().also { line = it } != null) {
                val currentLine = line ?: continue

                // SSE format: "data: {json}\n\n"
                if (currentLine.startsWith("data: ")) {
                    val jsonData = currentLine.removePrefix("data: ").trim()
                    if (jsonData.isNotEmpty()) {
                        try {
                            val section = jsonConverter.decodeFromString<SearchSection>(jsonData)
                            emit(section)

                            // Stop reading when done
                            if (section is SearchSection.Done) {
                                break
                            }
                        } catch (e: Exception) {
                            logger?.warn("SSE parsing error for data: $jsonData", e)
                        }
                    }
                }
                // Ignore other lines (empty lines, comments, etc.)
            }
        } finally {
            reader.close()
            body.close()
        }
    }.flowOn(Dispatchers.IO)

    private suspend fun <T> catchingNetworkError(block: suspend () -> RemoteApiResponse<T>): RemoteApiResponse<T> =
        try {
            block()
        } catch (t: Throwable) {
            // Distinguish between actual network errors and other errors (like JSON parsing)
            when (t) {
                is java.net.UnknownHostException,
                is java.net.ConnectException,
                is java.net.SocketTimeoutException,
                is java.net.SocketException,
                is javax.net.ssl.SSLException -> RemoteApiResponse.Error.Network
                else -> RemoteApiResponse.Error.Unknown(t.message ?: t.javaClass.simpleName)
            }
        }
}
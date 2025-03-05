package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import com.lelloman.pezzottify.android.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.remoteapi.response.TrackResponse
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import retrofit2.Response
import retrofit2.Retrofit
import retrofit2.awaitResponse
import retrofit2.converter.kotlinx.serialization.asConverterFactory

internal class RemoteApiClientImpl(
    baseUrl: String,
    okhttpClientBuilder: OkHttpClient.Builder,
    private val credentialsProvider: RemoteApiCredentialsProvider,
    private val coroutineDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : RemoteApiClient {

    private val okHttpClient = okhttpClientBuilder.build()
    private val authToken get() = credentialsProvider.authToken

    @OptIn(ExperimentalSerializationApi::class)
    private val jsonConverter = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
    }
    private val retrofit = Retrofit.Builder()
        .client(okHttpClient)
        .baseUrl(baseUrl)
        .addConverterFactory(jsonConverter.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(RetrofitApiClient::class.java)

    @Suppress("UNCHECKED_CAST")
    private fun <T> RemoteApiResponse.Error.cast() = this as RemoteApiResponse<T>

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

    private val <T>Response<T>.parsedBody: RemoteApiResponse<T>
        get() = try {
            RemoteApiResponse.Success(body()!!)
        } catch (t: Throwable) {
            RemoteApiResponse.Error.Unknown(t.message ?: "Unknown error").cast()
        }

    private fun <T> Response<T>.returnFromRetrofitResponse(): RemoteApiResponse<T> = commonError
        ?.cast()
        ?: parsedBody

    override suspend fun login(
        userHandle: String,
        password: String
    ): RemoteApiResponse<LoginSuccessResponse> = retrofit
        .login(LoginRequest(userHandle = userHandle, password = password))
        .returnFromRetrofitResponse()

    override suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse> =
        retrofit
            .getArtist(authToken = authToken, artistId = artistId)
            .returnFromRetrofitResponse()

    override suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse> =
        retrofit
            .getArtistDiscography(authToken = authToken, artistId = artistId)
            .returnFromRetrofitResponse()

    override suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse> = retrofit
        .getAlbum(authToken = authToken, albumId = albumId)
        .returnFromRetrofitResponse()

    override suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse> = retrofit
        .getTrack(authToken = authToken, trackId = trackId)
        .returnFromRetrofitResponse()

    override suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse> {
        val retrofitResponse = retrofit.getImage(authToken = authToken, imageId = imageId)
        retrofitResponse.commonError?.let { return it.cast() }
        val imageBytes = retrofitResponse.body()?.bytes()
            ?: return RemoteApiResponse.Error.Unknown("No body").cast()

        val mimeType = retrofitResponse.headers()["Content-Type"] ?: "image/*"

        return RemoteApiResponse.Success(ImageResponse(
            mimeType = mimeType,
            content = imageBytes,
        ))
    }

    override suspend fun search(
        query: String,
        filters: List<RemoteApiClient.SearchFilter>?
    ): RemoteApiResponse<SearchResponse> = retrofit
        .search(authToken, SearchRequest(query, filters?.map { it.name.lowercase() }))
        .returnFromRetrofitResponse()
}
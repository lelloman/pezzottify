package com.lelloman.pezzottify.remoteapi

import com.google.gson.GsonBuilder
import com.lelloman.pezzottify.remoteapi.internal.ArtistTypeAdapter
import com.lelloman.pezzottify.remoteapi.internal.HttpClient
import com.lelloman.pezzottify.remoteapi.internal.HttpClientImpl
import com.lelloman.pezzottify.remoteapi.internal.LoginRequest
import com.lelloman.pezzottify.remoteapi.model.Albums
import com.lelloman.pezzottify.remoteapi.model.Artist
import com.lelloman.pezzottify.remoteapi.model.Artists
import com.lelloman.pezzottify.remoteapi.model.UserStateResponse
import kotlinx.coroutines.withContext
import kotlin.coroutines.CoroutineContext

interface RemoteApi {

    suspend fun performLogin(
        remoteUrl: String,
        username: String,
        password: String
    ): Response<LoginResponse>

    suspend fun getUserState(): Response<UserStateResponse>

    suspend fun getAlbums(): Response<Albums>

    suspend fun getArtists(): Response<Artists>

    sealed class Response<T> {
        class Success<T>(val value: T) : Response<T>()
        class ResponseError<T> : Response<T>()
        class Unauthorized<T> : Response<T>()
        class NetworkError<T> : Response<T>()
        class ServerError<T> : Response<T>()
        class UnknownError<T> : Response<T>()
    }

    companion object Factory {
        fun create(ioContext: CoroutineContext): RemoteApi {
            val gson =
                GsonBuilder().registerTypeAdapter(Artist::class.java, ArtistTypeAdapter()).create()
            val httpClient = HttpClientImpl(gson)
            return RemoteApiImpl(httpClient, ioContext)
        }
    }
}

internal class RemoteApiImpl(
    private val httpClient: HttpClient,
    private val ioContext: CoroutineContext
) : RemoteApi {

    private var baseUrl = ""

    private suspend fun <T> onIo(action: () -> T) = withContext(ioContext) { action() }

    override suspend fun performLogin(
        remoteUrl: String, username: String, password: String
    ): RemoteApi.Response<LoginResponse> = onIo {
        val url = remoteUrl + LOGIN_PATH
        val response =
            httpClient.jsonPost(url, LoginRequest(username = username, password = password))
        if (response.isSuccessful) {
            val authToken = response.consumeStringBody()
            if (authToken == null) {
                RemoteApi.Response.ResponseError()
            } else {
                httpClient.setAuthToken(authToken)
                this.baseUrl = remoteUrl
                RemoteApi.Response.Success(LoginResponse.Success(authToken))
            }
        } else if (response.is4xx) {
            RemoteApi.Response.Success(LoginResponse.InvalidCredentials)
        } else {
            RemoteApi.Response.UnknownError()
        }
    }

    override suspend fun getUserState(): RemoteApi.Response<UserStateResponse> = onIo {
        val response = httpClient.get(baseUrl + USER_STATE_PATH)
        when {
            response.isSuccessful -> {
                try {
                    RemoteApi.Response.Success(response.consumeBody(UserStateResponse::class.java))
                } catch (_: Throwable) {
                    RemoteApi.Response.ResponseError()
                }
            }

            response.status == 401 || response.status == 403 -> RemoteApi.Response.Unauthorized()
            else -> RemoteApi.Response.UnknownError()
        }
    }

    override suspend fun getAlbums(): RemoteApi.Response<Albums> = onIo {
        val response = httpClient.get(baseUrl + ALBUMS_PATH)
        when {
            response.isSuccessful -> {
                try {
                    RemoteApi.Response.Success(response.consumeBody(Albums::class.java))
                } catch (_: Throwable) {
                    RemoteApi.Response.ResponseError()
                }
            }

            response.status == 401 || response.status == 403 -> RemoteApi.Response.Unauthorized()
            else -> RemoteApi.Response.UnknownError()
        }
    }

    override suspend fun getArtists(): RemoteApi.Response<Artists> = onIo {
        val response = httpClient.get(baseUrl + ARTISTS_PATH)
        when {
            response.isSuccessful -> {
                try {
                    RemoteApi.Response.Success(response.consumeBody(Artists::class.java))
                } catch (_: Throwable) {
                    RemoteApi.Response.ResponseError()
                }
            }

            response.status == 401 || response.status == 403 -> RemoteApi.Response.Unauthorized()
            else -> RemoteApi.Response.UnknownError()
        }
    }

    companion object {
        private const val LOGIN_PATH = "/api/auth"
        private const val USER_STATE_PATH = "/api/user/state"
        private const val ALBUMS_PATH = "/api/albums"
        private const val ARTISTS_PATH = "/api/artists"
    }
}
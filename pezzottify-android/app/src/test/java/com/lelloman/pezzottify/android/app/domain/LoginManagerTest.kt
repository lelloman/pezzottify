package com.lelloman.pezzottify.android.app.domain

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.remoteapi.LoginResponse
import com.lelloman.pezzottify.remoteapi.RemoteApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.mock
import org.mockito.kotlin.times
import org.mockito.kotlin.verify
import org.mockito.kotlin.verifyNoInteractions
import org.mockito.kotlin.whenever
import java.io.File
import java.net.ConnectException

class LoginManagerTest {

    private var persistenceFile: File = mock()
    private val remoteApi: RemoteApi = mock()

    private val tested by lazy {
        LoginManagerImpl(
            persistence = persistenceFile,
            remoteApi = remoteApi,
            ioDispatcher = Dispatchers.IO,
            loginOperations = emptySet(),
            logoutOperations = emptySet(),
        )
    }

    @Test
    fun `loads persistence state when loginState is observed`() {
        runBlocking {
            assertThat(tested).isNotNull()
            verifyNoInteractions(persistenceFile)

            assertThat(tested.loginState.first()).isEqualTo(LoginState.Unauthenticated)

            verify(persistenceFile, times(1)).path
        }
    }

    @Test
    fun `loads logged in state from persistence file`() {
        runBlocking {
            persistenceFile = File.createTempFile("login", "persistence")
            val remoteUrl = "http://asd.com"
            val username = "the username"
            val token = "the token"
            persistenceFile.writeText("$remoteUrl\n$username\n$token")

            val state = tested.loginState.first()

            assertThat(state).isEqualTo(
                LoginState.LoggedIn(
                    username = username,
                    authToken = token,
                    remoteUrl = remoteUrl
                )
            )
        }
    }

    @Test
    fun `returns network error`() {
        runBlocking {
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenAnswer { throw ConnectException("networky stuff") }

            assertThat(tested.performLogin("", "", "")).isEqualTo(LoginResult.Failure.Network)
        }
    }

    @Test
    fun `returns invalid credentials error`() {
        runBlocking {
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenReturn(RemoteApi.Response.Success(LoginResponse.InvalidCredentials))

            assertThat(tested.performLogin("", "", "")).isEqualTo(LoginResult.Failure.Credentials)
        }
    }

    @Test
    fun `returns unknown error`() {
        runBlocking {
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenAnswer { throw IllegalStateException("mweh") }

            assertThat(tested.performLogin("", "", "")).isEqualTo(LoginResult.Failure.Unknown)
        }
    }

    @Test
    fun `returns invalid credentials error 2`() {
        runBlocking {
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenReturn(RemoteApi.Response.UnknownError())

            assertThat(tested.performLogin("", "", "")).isEqualTo(LoginResult.Failure.Unknown)
        }
    }

    @Test
    fun `returns invalid credentials error 3`() {
        runBlocking {
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenReturn(RemoteApi.Response.ResponseError())

            assertThat(tested.performLogin("", "", "")).isEqualTo(LoginResult.Failure.Unknown)
        }
    }

    @Test
    fun `logs in successfully`() {
        runBlocking {
            val (username, token, remoteUrl) = arrayOf("Username", "Token", "http://asd.com")
            persistenceFile = File.createTempFile("persitence", "tmp")
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenReturn(RemoteApi.Response.Success(LoginResponse.Success(token)))

            assertThat(
                tested.performLogin(
                    username = username,
                    remoteUrl = remoteUrl,
                    password = ""
                )
            ).isEqualTo(LoginResult.Success(token))
            assertThat(tested.loginState.first())
                .isEqualTo(
                    LoginState.LoggedIn(
                        username = username,
                        authToken = token,
                        remoteUrl = remoteUrl
                    )
                )
            assertThat(persistenceFile.readText()).isEqualTo("$remoteUrl\n$username\n$token")
        }
    }
}
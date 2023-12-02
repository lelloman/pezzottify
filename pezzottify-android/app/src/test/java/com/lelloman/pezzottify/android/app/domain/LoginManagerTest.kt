package com.lelloman.pezzottify.android.app.domain

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.app.domain.login.LoginManagerImpl
import com.lelloman.pezzottify.android.app.domain.login.LoginResult
import com.lelloman.pezzottify.android.app.domain.login.LoginState
import com.lelloman.pezzottify.android.app.localdata.ObjectsStore
import com.lelloman.pezzottify.remoteapi.LoginResponse
import com.lelloman.pezzottify.remoteapi.RemoteApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.doReturn
import org.mockito.kotlin.mock
import org.mockito.kotlin.times
import org.mockito.kotlin.verify
import org.mockito.kotlin.verifyNoInteractions
import org.mockito.kotlin.whenever
import java.net.ConnectException

class LoginManagerTest {

    private val remoteApi: RemoteApi = mock()
    private val objectsStore: ObjectsStore = mock()

    private val tested by lazy {
        LoginManagerImpl(
            objectsStore = objectsStore,
            remoteApi = remoteApi,
            ioDispatcher = Dispatchers.IO,
        )
    }

    @Test
    fun `loads persistence state when loginState is observed`() {
        runBlocking {
            assertThat(tested).isNotNull()
            whenever(objectsStore.load(LoginManagerImpl.persistenceObjectDef)).thenThrow(
                IllegalArgumentException()
            )
            verifyNoInteractions(objectsStore)

            assertThat(tested.loginState.first()).isEqualTo(LoginState.Unauthenticated)

            verify(objectsStore, times(1)).load(LoginManagerImpl.persistenceObjectDef)
        }
    }

    @Test
    fun `loads logged in state from persistence file`() {
        runBlocking {
            val persistedState = LoginState.LoggedIn(
                username = "the username",
                remoteUrl = "http://asd.com",
                authToken = "the token",
            )
            doReturn(persistedState).whenever(objectsStore)
                .load(LoginManagerImpl.persistenceObjectDef)

            val state = tested.loginState.first()

            assertThat(state).isEqualTo(persistedState)
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
            whenever(remoteApi.performLogin(any(), any(), any()))
                .thenReturn(RemoteApi.Response.Success(LoginResponse.Success(token)))
            whenever(objectsStore.load(LoginManagerImpl.persistenceObjectDef)).thenThrow(
                IllegalArgumentException()
            )
            assertThat(tested.loginState.first())
                .isEqualTo(LoginState.Unauthenticated)

            assertThat(
                tested.performLogin(
                    username = username,
                    remoteUrl = remoteUrl,
                    password = ""
                )
            ).isEqualTo(LoginResult.Success(token))
            val loggedInState = LoginState.LoggedIn(
                username = username,
                authToken = token,
                remoteUrl = remoteUrl
            )
            assertThat(tested.loginState.first())
                .isEqualTo(loggedInState)
            verify(objectsStore).store(LoginManagerImpl.persistenceObjectDef, loggedInState)
        }
    }
}
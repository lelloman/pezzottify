package com.lelloman.pezzottify.android.domain.auth.usecase

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coVerify
import io.mockk.coVerifyOrder
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class PerformDeviceFlowLoginTest {

    private lateinit var authStore: AuthStore
    private lateinit var configStore: ConfigStore
    private lateinit var syncManager: SyncManager
    private lateinit var webSocketManager: WebSocketManager
    private lateinit var performDeviceFlowLogin: PerformDeviceFlowLogin

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        configStore = mockk(relaxed = true)
        syncManager = mockk(relaxed = true)
        webSocketManager = mockk(relaxed = true)
        every { configStore.baseUrl } returns MutableStateFlow("https://music.example.com")

        val logger = mockk<Logger>(relaxed = true)
        val loggerFactory = mockk<LoggerFactory>()
        every { loggerFactory.getLogger(any<String>()) } returns logger
        every { loggerFactory.getValue(any(), any()) } returns logger

        performDeviceFlowLogin = PerformDeviceFlowLogin(
            PerformOidcLogin(authStore, configStore, syncManager, webSocketManager, loggerFactory)
        )
    }

    @Test
    fun `approved tokens are stored as an OIDC session for the configured host`() = runTest {
        val result = performDeviceFlowLogin(
            DeviceFlowAuthManager.PollResult.Approved(
                idToken = "id-token",
                refreshToken = "refresh-token",
                userHandle = "user@example.com",
            )
        )

        assertThat(result).isEqualTo(PerformOidcLogin.LoginResult.Success)
        coVerify {
            authStore.storeAuthState(
                AuthState.LoggedIn(
                    userHandle = "user@example.com",
                    authToken = "id-token",
                    refreshToken = "refresh-token",
                    remoteUrl = "https://music.example.com",
                )
            )
        }
    }

    @Test
    fun `stores the session before connecting the websocket and starting sync`() = runTest {
        performDeviceFlowLogin(
            DeviceFlowAuthManager.PollResult.Approved(
                idToken = "id-token",
                refreshToken = null,
                userHandle = "user",
            )
        )

        coVerifyOrder {
            authStore.storeAuthState(any())
            webSocketManager.connect()
            syncManager.initialize()
        }
    }

    @Test
    fun `a missing refresh token is stored as null`() = runTest {
        performDeviceFlowLogin(
            DeviceFlowAuthManager.PollResult.Approved(
                idToken = "id-token",
                refreshToken = null,
                userHandle = "user",
            )
        )

        coVerify {
            authStore.storeAuthState(match { it is AuthState.LoggedIn && it.refreshToken == null })
        }
    }
}

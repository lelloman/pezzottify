package com.lelloman.pezzottify.android.domain.auth.usecase

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredEventBus
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class HandleSessionExpiredTest {

    private lateinit var authStore: AuthStore
    private lateinit var staticsStore: StaticsStore
    private lateinit var staticsCache: StaticsCache
    private lateinit var skeletonStore: SkeletonStore
    private lateinit var userDataStore: UserDataStore
    private lateinit var userContentStore: UserContentStore
    private lateinit var permissionsStore: PermissionsStore
    private lateinit var listeningEventStore: ListeningEventStore
    private lateinit var syncManager: SyncManager
    private lateinit var player: PezzottifyPlayer
    private lateinit var webSocketManager: WebSocketManager
    private lateinit var sessionExpiredEventBus: SessionExpiredEventBus
    private lateinit var loggerFactory: LoggerFactory

    private lateinit var handleSessionExpired: HandleSessionExpired

    private val authStateFlow = MutableStateFlow<AuthState>(loggedInState())

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        staticsStore = mockk(relaxed = true)
        staticsCache = mockk(relaxed = true)
        skeletonStore = mockk(relaxed = true)
        userDataStore = mockk(relaxed = true)
        userContentStore = mockk(relaxed = true)
        permissionsStore = mockk(relaxed = true)
        listeningEventStore = mockk(relaxed = true)
        syncManager = mockk(relaxed = true)
        player = mockk(relaxed = true)
        webSocketManager = mockk(relaxed = true)
        sessionExpiredEventBus = SessionExpiredEventBus()

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger
        every { authStore.getAuthState() } returns authStateFlow

        handleSessionExpired = HandleSessionExpired(
            authStore = authStore,
            staticsStore = staticsStore,
            staticsCache = staticsCache,
            skeletonStore = skeletonStore,
            userDataStore = userDataStore,
            userContentStore = userContentStore,
            permissionsStore = permissionsStore,
            listeningEventStore = listeningEventStore,
            syncManager = syncManager,
            player = player,
            webSocketManager = webSocketManager,
            sessionExpiredEventBus = sessionExpiredEventBus,
            loggerFactory = loggerFactory,
        )
    }

    private fun loggedInState() = AuthState.LoggedIn(
        userHandle = "test-user",
        authToken = "test-token",
        remoteUrl = "http://localhost:3001"
    )

    @Test
    fun `invoke clears player session when logged in`() = runTest {
        handleSessionExpired()

        verify { player.clearSession() }
    }

    @Test
    fun `invoke sets auth state to logged out when logged in`() = runTest {
        handleSessionExpired()

        coVerify { authStore.storeAuthState(AuthState.LoggedOut) }
    }

    @Test
    fun `invoke clears in-memory cache when logged in`() = runTest {
        handleSessionExpired()

        verify { staticsCache.clearAll() }
    }

    @Test
    fun `invoke deletes all statics when logged in`() = runTest {
        handleSessionExpired()

        coVerify { staticsStore.deleteAll() }
    }

    @Test
    fun `invoke clears skeleton store when logged in`() = runTest {
        handleSessionExpired()

        coVerify { skeletonStore.clear() }
    }

    @Test
    fun `invoke deletes all user data when logged in`() = runTest {
        handleSessionExpired()

        coVerify { userDataStore.deleteAll() }
    }

    @Test
    fun `invoke deletes all user content when logged in`() = runTest {
        handleSessionExpired()

        coVerify { userContentStore.deleteAll() }
    }

    @Test
    fun `invoke deletes all listening events when logged in`() = runTest {
        handleSessionExpired()

        coVerify { listeningEventStore.deleteAll() }
    }

    @Test
    fun `invoke disconnects websocket when logged in`() = runTest {
        handleSessionExpired()

        coVerify { webSocketManager.disconnect() }
    }

    @Test
    fun `invoke cleans up sync manager when logged in`() = runTest {
        handleSessionExpired()

        coVerify { syncManager.cleanup() }
    }

    @Test
    fun `invoke clears permissions store when logged in`() = runTest {
        handleSessionExpired()

        coVerify { permissionsStore.clear() }
    }

    @Test
    fun `invoke skips cleanup when already logged out`() = runTest {
        authStateFlow.value = AuthState.LoggedOut

        handleSessionExpired()

        // No cleanup should happen
        verify(exactly = 0) { player.clearSession() }
        coVerify(exactly = 0) { authStore.storeAuthState(any()) }
        coVerify(exactly = 0) { staticsStore.deleteAll() }
    }

    @Test
    fun `invoke calls all cleanup operations when logged in`() = runTest {
        handleSessionExpired()

        coVerify {
            webSocketManager.disconnect()
            syncManager.cleanup()
            authStore.storeAuthState(AuthState.LoggedOut)
            staticsStore.deleteAll()
            skeletonStore.clear()
            userDataStore.deleteAll()
            userContentStore.deleteAll()
            permissionsStore.clear()
            listeningEventStore.deleteAll()
        }
        verify {
            player.clearSession()
            staticsCache.clearAll()
        }
    }

    @Test
    fun `events flow exposes session expired events from bus`() = runTest {
        val events = handleSessionExpired.events

        assertThat(events).isSameInstanceAs(sessionExpiredEventBus.events)
    }
}

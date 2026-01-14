package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
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

class PerformLogoutTest {

    private lateinit var authStore: AuthStore
    private lateinit var remoteApiClient: RemoteApiClient
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
    private lateinit var loggerFactory: LoggerFactory

    private lateinit var performLogout: PerformLogout

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        remoteApiClient = mockk(relaxed = true)
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

        // Mock auth state for login hint feature
        val authStateFlow = MutableStateFlow<AuthState>(
            AuthState.LoggedIn(
                userHandle = "test-user",
                authToken = "test-token",
                refreshToken = "test-refresh-token",
                remoteUrl = "http://localhost:3001"
            )
        )
        every { authStore.getAuthState() } returns authStateFlow

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        performLogout = PerformLogout(
            authStore = authStore,
            remoteApiClient = remoteApiClient,
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
            loggerFactory = loggerFactory,
        )
    }

    @Test
    fun `invoke clears player session`() = runTest {
        performLogout()

        verify { player.clearSession() }
    }

    @Test
    fun `invoke sets auth state to logged out`() = runTest {
        performLogout()

        coVerify { authStore.storeAuthState(AuthState.LoggedOut) }
    }

    @Test
    fun `invoke calls remote logout`() = runTest {
        performLogout()

        coVerify { remoteApiClient.logout() }
    }

    @Test
    fun `invoke clears in-memory cache`() = runTest {
        performLogout()

        verify { staticsCache.clearAll() }
    }

    @Test
    fun `invoke deletes all statics`() = runTest {
        performLogout()

        coVerify { staticsStore.deleteAll() }
    }

    @Test
    fun `invoke clears skeleton store`() = runTest {
        performLogout()

        coVerify { skeletonStore.clear() }
    }

    @Test
    fun `invoke deletes all user data`() = runTest {
        performLogout()

        coVerify { userDataStore.deleteAll() }
    }

    @Test
    fun `invoke deletes all user content`() = runTest {
        performLogout()

        coVerify { userContentStore.deleteAll() }
    }

    @Test
    fun `invoke deletes all listening events`() = runTest {
        performLogout()

        coVerify { listeningEventStore.deleteAll() }
    }

    @Test
    fun `invoke disconnects websocket`() = runTest {
        performLogout()

        coVerify { webSocketManager.disconnect() }
    }

    @Test
    fun `invoke cleans up sync manager`() = runTest {
        performLogout()

        coVerify { syncManager.cleanup() }
    }

    @Test
    fun `invoke clears permissions store`() = runTest {
        performLogout()

        coVerify { permissionsStore.clear() }
    }

    @Test
    fun `invoke calls all cleanup operations`() = runTest {
        performLogout()

        coVerify {
            webSocketManager.disconnect()
            syncManager.cleanup()
            authStore.storeAuthState(AuthState.LoggedOut)
            remoteApiClient.logout()
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
    fun `invoke saves last used handle for login hint`() = runTest {
        performLogout()

        coVerify { authStore.storeLastUsedHandle("test-user") }
    }

    @Test
    fun `invoke does not save last used handle when logged out`() = runTest {
        // Setup: auth state is LoggedOut
        val authStateFlow = MutableStateFlow<AuthState>(AuthState.LoggedOut)
        every { authStore.getAuthState() } returns authStateFlow

        performLogout()

        coVerify(exactly = 0) { authStore.storeLastUsedHandle(any()) }
    }

    @Test
    fun `invoke does not save last used handle when loading`() = runTest {
        // Setup: auth state is Loading
        val authStateFlow = MutableStateFlow<AuthState>(AuthState.Loading)
        every { authStore.getAuthState() } returns authStateFlow

        performLogout()

        coVerify(exactly = 0) { authStore.storeLastUsedHandle(any()) }
    }
}

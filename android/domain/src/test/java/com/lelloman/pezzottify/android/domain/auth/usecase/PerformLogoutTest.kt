package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class PerformLogoutTest {

    private lateinit var authStore: AuthStore
    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var staticsStore: StaticsStore
    private lateinit var userDataStore: UserDataStore
    private lateinit var userContentStore: UserContentStore
    private lateinit var player: PezzottifyPlayer

    private lateinit var performLogout: PerformLogout

    @Before
    fun setUp() {
        authStore = mockk(relaxed = true)
        remoteApiClient = mockk(relaxed = true)
        staticsStore = mockk(relaxed = true)
        userDataStore = mockk(relaxed = true)
        userContentStore = mockk(relaxed = true)
        player = mockk(relaxed = true)

        performLogout = PerformLogout(
            authStore = authStore,
            remoteApiClient = remoteApiClient,
            staticsStore = staticsStore,
            userDataStore = userDataStore,
            userContentStore = userContentStore,
            player = player,
        )
    }

    @Test
    fun `invoke stops player`() = runTest {
        performLogout()

        coVerify { player.stop() }
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
    fun `invoke deletes all statics`() = runTest {
        performLogout()

        coVerify { staticsStore.deleteAll() }
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
    fun `invoke calls all cleanup operations`() = runTest {
        performLogout()

        coVerify {
            player.stop()
            authStore.storeAuthState(AuthState.LoggedOut)
            remoteApiClient.logout()
            staticsStore.deleteAll()
            userDataStore.deleteAll()
            userContentStore.deleteAll()
        }
    }
}

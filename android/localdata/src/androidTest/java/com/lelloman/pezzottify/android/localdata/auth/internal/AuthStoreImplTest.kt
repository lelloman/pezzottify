package com.lelloman.pezzottify.android.localdata.auth.internal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.localdata.auth.AuthState
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AuthStoreImplTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        context.deleteSharedPreferences(AuthStoreImpl.SHARED_PREF_FILE_NAME)
    }

    private fun withTested(test: suspend TestScope.(AuthStoreImpl) -> Unit) = runTest {
        val authStore = AuthStoreImpl(
            context,
            dispatcher = UnconfinedTestDispatcher(testScheduler)
        )
        test(authStore)
    }

    @Test
    fun hasLoadingStateBeforeInitialization() = withTested { authStore ->
        val authState = authStore.getAuthState().value
        assertThat(authState).isInstanceOf(AuthState.Loading::class.java)
    }

    @Test
    fun hasLoggedOutStateAfterInitialization() = withTested { authStore ->
        authStore.initialize()

        advanceUntilIdle()
        val authState = authStore.getAuthState().value
        assertThat(authState).isInstanceOf(AuthState.LoggedOut::class.java)
    }

    @Test
    fun persistLoggedInState() = withTested { authStore ->
        authStore.initialize()
        advanceUntilIdle()
        val authState = authStore.getAuthState().value
        assertThat(authState).isInstanceOf(AuthState.LoggedOut::class.java)

        authStore.storeAuthState(AuthState.LoggedIn("userHandle", "authToken", "remoteUrl"))
        advanceUntilIdle()

        val authStateAfter = authStore.getAuthState().value
        assertThat(authStateAfter).isInstanceOf(AuthState.LoggedIn::class.java)
    }
}

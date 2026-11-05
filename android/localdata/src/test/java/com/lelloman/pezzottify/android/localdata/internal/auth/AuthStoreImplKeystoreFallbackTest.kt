package com.lelloman.pezzottify.android.localdata.internal.auth

import android.content.Context
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.localdata.internal.security.KeystoreFallback
import com.lelloman.pezzottify.android.localdata.internal.security.SecurePreferencesFactory
import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

// Robolectric has no AndroidKeyStore, so the real MasterKey and the real probe both fail there,
// like on a box whose keymaster is broken: AuthStoreImpl must still start and keep its state.
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AuthStoreImplKeystoreFallbackTest {

    @get:Rule val folder = TemporaryFolder()

    private val context: Context = RuntimeEnvironment.getApplication()
    private val loggerFactory = LoggerFactory(MutableStateFlow(LogLevel.None))
    private val marker get() = folder.root.resolve(KeystoreFallback.MARKER_FILE_NAME)

    private fun keystoreFallback() = KeystoreFallback(marker, loggerFactory.getLogger("test"))

    @Test
    fun `starts without a keystore and keeps the auth state in plain preferences`() = runTest {
        val dispatcher = UnconfinedTestDispatcher(testScheduler)
        val first = AuthStoreImpl(context, this, loggerFactory, dispatcher, keystoreFallback())
        first.storeLastUsedHandle("user")

        assertThat(marker.exists()).isTrue()
        val plain = context.getSharedPreferences(
            SecurePreferencesFactory.plainName(AuthStoreImpl.SHARED_PREF_FILE_NAME),
            Context.MODE_PRIVATE,
        )
        assertThat(plain.getString("LastUsedHandle", null)).isEqualTo("user")

        val second = AuthStoreImpl(context, this, loggerFactory, dispatcher, keystoreFallback())
        second.initialize()
        advanceUntilIdle()
        assertThat(second.getLastUsedHandle()).isEqualTo("user")
        assertThat(second.getAuthState().value).isEqualTo(AuthState.LoggedOut)
    }
}

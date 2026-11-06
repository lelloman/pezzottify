package com.lelloman.pezzottify.android.localdata.internal.config

import android.content.Context
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore.SetBaseUrlResult
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ConfigStoreImplTest {

    private val context: Context = RuntimeEnvironment.getApplication()

    @Before
    fun setUp() {
        context.getSharedPreferences(ConfigStoreImpl.SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    private fun store(configurable: Boolean, dispatcher: TestDispatcher) = ConfigStoreImpl(
        context = context,
        defaultHostUrl = DEFAULT,
        serverConfigurable = configurable,
        dispatcher = dispatcher,
    )

    @Test
    fun `configurable store keeps the saved host`() = runTest {
        val dispatcher = UnconfinedTestDispatcher(testScheduler)
        assertThat(store(true, dispatcher).setBaseUrl(SAVED)).isEqualTo(SetBaseUrlResult.Success)

        assertThat(store(true, dispatcher).baseUrl.value).isEqualTo(SAVED)
    }

    @Test
    fun `pinned store ignores a host saved by an earlier configurable build`() = runTest {
        val dispatcher = UnconfinedTestDispatcher(testScheduler)
        store(true, dispatcher).setBaseUrl(SAVED)

        assertThat(store(false, dispatcher).baseUrl.value).isEqualTo(DEFAULT)
    }

    @Test
    fun `pinned store reports success for any host but never changes`() = runTest {
        val dispatcher = UnconfinedTestDispatcher(testScheduler)
        val store = store(false, dispatcher)

        assertThat(store.setBaseUrl(SAVED)).isEqualTo(SetBaseUrlResult.Success)
        assertThat(store.setBaseUrl("not a url")).isEqualTo(SetBaseUrlResult.Success)
        assertThat(store.baseUrl.value).isEqualTo(DEFAULT)
        assertThat(store(true, dispatcher).baseUrl.value).isEqualTo(DEFAULT)
    }

    private companion object {
        const val DEFAULT = "https://pinned.example.com"
        const val SAVED = "https://other.example.com"
    }
}

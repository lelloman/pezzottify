package com.lelloman.pezzottify.android.ui.screen.maintenance

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.response.ServerStatus
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class MaintenanceViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `logout keeps phone and TV on the blocking screen while maintenance remains enabled`() =
        runTest {
            val interactor = FakeInteractor(
                status = ServerStatus(enabled = true, message = "Upgrading"),
                isLoggedIn = true,
            )
            val viewModel = MaintenanceViewModel(interactor)
            val events = mutableListOf<MaintenanceScreenEvent>()
            val collector = launch(UnconfinedTestDispatcher(testScheduler)) {
                viewModel.events.collect { events.add(it) }
            }
            advanceUntilIdle()

            assertThat(viewModel.state.value.isLoggedIn).isTrue()

            viewModel.logout()
            advanceUntilIdle()

            assertThat(interactor.logoutCalls).isEqualTo(1)
            assertThat(viewModel.state.value.isLoggedIn).isFalse()
            assertThat(viewModel.state.value.isLoggingOut).isFalse()
            assertThat(viewModel.state.value.error).isFalse()
            assertThat(events).isEmpty()
            collector.cancel()
        }

    @Test
    fun `logout navigates to login only after a status probe confirms maintenance ended`() =
        runTest {
            val interactor = FakeInteractor(
                status = ServerStatus(enabled = true),
                isLoggedIn = true,
            )
            val viewModel = MaintenanceViewModel(interactor)
            val events = mutableListOf<MaintenanceScreenEvent>()
            val collector = launch(UnconfinedTestDispatcher(testScheduler)) {
                viewModel.events.collect { events.add(it) }
            }
            advanceUntilIdle()

            viewModel.logout()
            advanceUntilIdle()

            assertThat(events).isEmpty()

            interactor.status = ServerStatus(enabled = false)
            viewModel.refresh()
            advanceUntilIdle()

            assertThat(events).containsExactly(
                MaintenanceScreenEvent.MaintenanceEnded(isLoggedIn = false),
            )
            collector.cancel()
        }

    @Test
    fun `failed status probe keeps the user on the blocking screen after logout`() = runTest {
        val interactor = FakeInteractor(
            status = ServerStatus(enabled = true),
            isLoggedIn = true,
        )
        val viewModel = MaintenanceViewModel(interactor)
        val events = mutableListOf<MaintenanceScreenEvent>()
        val collector = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }
        advanceUntilIdle()

        interactor.status = null
        viewModel.logout()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoggedIn).isFalse()
        assertThat(viewModel.state.value.error).isTrue()
        assertThat(events).isEmpty()
        collector.cancel()
    }

    private class FakeInteractor(
        var status: ServerStatus?,
        var isLoggedIn: Boolean,
    ) : MaintenanceViewModel.Interactor {
        var logoutCalls = 0

        override suspend fun fetchStatus(): ServerStatus? = status

        override suspend fun isLoggedIn(): Boolean = isLoggedIn

        override suspend fun logout() {
            logoutCalls++
            isLoggedIn = false
        }
    }
}

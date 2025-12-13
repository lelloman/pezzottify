package com.lelloman.pezzottify.android.ui.screen.splash

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SplashViewModelTest {

    @Test
    fun `destination is Main when logged in`() = runTest {
        val fakeInteractor = FakeInteractor(isLoggedIn = true)
        val viewModel = SplashViewModel(fakeInteractor)

        val destination = viewModel.destination.first()

        assertThat(destination).isEqualTo(SplashViewModel.Destination.Main)
    }

    @Test
    fun `destination is Login when not logged in`() = runTest {
        val fakeInteractor = FakeInteractor(isLoggedIn = false)
        val viewModel = SplashViewModel(fakeInteractor)

        val destination = viewModel.destination.first()

        assertThat(destination).isEqualTo(SplashViewModel.Destination.Login)
    }

    private class FakeInteractor(
        private val isLoggedIn: Boolean
    ) : SplashViewModel.Interactor {
        override suspend fun isLoggedIn(): Boolean = isLoggedIn
    }
}

package com.lelloman.pezzottify.android.ui.screen.login

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.R
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
class LoginViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: LoginViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        viewModel = LoginViewModel(fakeInteractor)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `initial state uses values from interactor`() {
        fakeInteractor.setInitialHost("https://example.com")
        fakeInteractor.setInitialEmail("user@test.com")

        viewModel = LoginViewModel(fakeInteractor)

        assertThat(viewModel.state.value.host).isEqualTo("https://example.com")
        assertThat(viewModel.state.value.email).isEqualTo("user@test.com")
        assertThat(viewModel.state.value.password).isEmpty()
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `updateHost updates state and clears host error`() {
        viewModel.updateHost("https://new-host.com")

        assertThat(viewModel.state.value.host).isEqualTo("https://new-host.com")
        assertThat(viewModel.state.value.hostErrorRes).isNull()
    }

    @Test
    fun `updateEmail updates state`() {
        viewModel.updateEmail("new@email.com")

        assertThat(viewModel.state.value.email).isEqualTo("new@email.com")
    }

    @Test
    fun `updatePassword updates state`() {
        viewModel.updatePassword("secret123")

        assertThat(viewModel.state.value.password).isEqualTo("secret123")
    }

    @Test
    fun `clickOnLoginButton does nothing when already loading`() = runTest {
        // Set up a slow interactor that we can control
        fakeInteractor.loginDelay = true
        viewModel.updateHost("https://host.com")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("password")

        // Start first login attempt (launches coroutine in viewModelScope)
        viewModel.clockOnLoginButton()

        // Advance past the setHost call but pause at the login delay
        testDispatcher.scheduler.advanceUntilIdle()

        // While loading, try another click
        assertThat(viewModel.state.value.isLoading).isTrue()
        viewModel.clockOnLoginButton()
        testDispatcher.scheduler.advanceUntilIdle()

        // Only one login call should have been made
        assertThat(fakeInteractor.loginCallCount).isEqualTo(1)

        // Complete and clean up
        fakeInteractor.completeLogin()
        testDispatcher.scheduler.advanceUntilIdle()
    }

    @Test
    fun `clickOnLoginButton with invalid URL sets host error`() = runTest {
        fakeInteractor.setHostResult = LoginViewModel.Interactor.SetHostResult.InvalidUrl
        viewModel.updateHost("not-a-url")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("password")

        viewModel.clockOnLoginButton()
        advanceUntilIdle()

        assertThat(viewModel.state.value.hostErrorRes).isEqualTo(R.string.invalid_url)
        assertThat(viewModel.state.value.isLoading).isFalse()
        assertThat(fakeInteractor.loginCallCount).isEqualTo(0)
    }

    @Test
    fun `clickOnLoginButton with invalid credentials sets error`() = runTest {
        fakeInteractor.loginResult = LoginViewModel.Interactor.LoginResult.Failure.InvalidCredentials
        viewModel.updateHost("https://host.com")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("wrong")

        viewModel.clockOnLoginButton()
        advanceUntilIdle()

        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.invalid_credentials)
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `clickOnLoginButton with unknown error sets error`() = runTest {
        fakeInteractor.loginResult = LoginViewModel.Interactor.LoginResult.Failure.Unknown
        viewModel.updateHost("https://host.com")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("password")

        viewModel.clockOnLoginButton()
        advanceUntilIdle()

        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.unknown_error)
        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    @Test
    fun `clickOnLoginButton success emits RequestNotificationPermission and NavigateToMain events`() = runTest {
        fakeInteractor.loginResult = LoginViewModel.Interactor.LoginResult.Success
        viewModel.updateHost("https://host.com")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("password")

        val events = mutableListOf<LoginScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clockOnLoginButton()
        advanceUntilIdle()

        assertThat(events).containsExactly(
            LoginScreenEvents.RequestNotificationPermission,
            LoginScreenEvents.NavigateToMain
        ).inOrder()
        assertThat(viewModel.state.value.isLoading).isFalse()

        job.cancel()
    }

    @Test
    fun `clickOnLoginButton passes correct credentials to interactor`() = runTest {
        viewModel.updateHost("https://my-server.com")
        viewModel.updateEmail("test@example.com")
        viewModel.updatePassword("mypassword")

        viewModel.clockOnLoginButton()
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetHost).isEqualTo("https://my-server.com")
        assertThat(fakeInteractor.lastLoginEmail).isEqualTo("test@example.com")
        assertThat(fakeInteractor.lastLoginPassword).isEqualTo("mypassword")
    }

    @Test
    fun `loading state is true during login`() = runTest {
        fakeInteractor.loginDelay = true
        viewModel.updateHost("https://host.com")
        viewModel.updateEmail("user@test.com")
        viewModel.updatePassword("password")

        viewModel.clockOnLoginButton()

        // Advance scheduler so the coroutine runs up to the suspension point
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isTrue()

        fakeInteractor.completeLogin()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoading).isFalse()
    }

    private class FakeInteractor : LoginViewModel.Interactor {
        private var _initialHost = ""
        private var _initialEmail = ""

        fun setInitialHost(host: String) { _initialHost = host }
        fun setInitialEmail(email: String) { _initialEmail = email }

        var setHostResult: LoginViewModel.Interactor.SetHostResult =
            LoginViewModel.Interactor.SetHostResult.Success
        var loginResult: LoginViewModel.Interactor.LoginResult =
            LoginViewModel.Interactor.LoginResult.Success

        var loginDelay = false
        private var loginContinuation: (() -> Unit)? = null

        var loginCallCount = 0
        var lastSetHost: String? = null
        var lastLoginEmail: String? = null
        var lastLoginPassword: String? = null

        override fun getInitialHost(): String = _initialHost

        override fun getInitialEmail(): String = _initialEmail

        override suspend fun setHost(host: String): LoginViewModel.Interactor.SetHostResult {
            lastSetHost = host
            return setHostResult
        }

        override suspend fun login(
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult {
            loginCallCount++
            lastLoginEmail = email
            lastLoginPassword = password

            if (loginDelay) {
                kotlinx.coroutines.suspendCancellableCoroutine { cont ->
                    loginContinuation = { cont.resume(Unit) {} }
                }
            }

            return loginResult
        }

        fun completeLogin() {
            loginContinuation?.invoke()
        }
    }
}

package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.model.Permission
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
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
class ProfileScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: ProfileScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel() {
        viewModel = ProfileScreenViewModel(fakeInteractor)
    }

    @Test
    fun `initial state loads user info from interactor`() = runTest {
        fakeInteractor.setUserName("John Doe")
        fakeInteractor.setBaseUrl("https://api.example.com")
        fakeInteractor.setBuildVariant("debug")
        fakeInteractor.setVersionName("1.2.3")
        fakeInteractor.setGitCommit("abc123")

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.userName).isEqualTo("John Doe")
        assertThat(viewModel.state.value.baseUrl).isEqualTo("https://api.example.com")
        assertThat(viewModel.state.value.buildVariant).isEqualTo("debug")
        assertThat(viewModel.state.value.versionName).isEqualTo("1.2.3")
        assertThat(viewModel.state.value.gitCommit).isEqualTo("abc123")
    }

    @Test
    fun `state updates when server version changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.serverVersionFlow.value = "2.0.0"
        advanceUntilIdle()

        assertThat(viewModel.state.value.serverVersion).isEqualTo("2.0.0")
    }

    @Test
    fun `state updates when permissions change`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val permissions = setOf(Permission.AccessCatalog, Permission.EditCatalog)
        fakeInteractor.permissionsFlow.value = permissions
        advanceUntilIdle()

        assertThat(viewModel.state.value.permissions).isEqualTo(permissions)
    }

    @Test
    fun `clickOnLogout shows confirmation dialog`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.clickOnLogout()

        assertThat(viewModel.state.value.showLogoutConfirmation).isTrue()
    }

    @Test
    fun `clickOnLogout does nothing when already logging out`() = runTest {
        createViewModel()
        advanceUntilIdle()

        // Simulate already logging out by confirming logout (sets isLoggingOut = true)
        fakeInteractor.logoutDelay = true
        viewModel.clickOnLogout()
        viewModel.confirmLogout()
        advanceUntilIdle()

        assertThat(viewModel.state.value.isLoggingOut).isTrue()

        // Try to click logout again - should not show confirmation
        viewModel.clickOnLogout()

        assertThat(viewModel.state.value.showLogoutConfirmation).isFalse()

        fakeInteractor.completeLogout()
        advanceUntilIdle()
    }

    @Test
    fun `dismissLogoutConfirmation hides confirmation dialog`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.clickOnLogout()
        assertThat(viewModel.state.value.showLogoutConfirmation).isTrue()

        viewModel.dismissLogoutConfirmation()

        assertThat(viewModel.state.value.showLogoutConfirmation).isFalse()
    }

    @Test
    fun `confirmLogout calls interactor logout and emits navigation event`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val events = mutableListOf<ProfileScreenEvents>()
        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            viewModel.events.collect { events.add(it) }
        }

        viewModel.clickOnLogout()
        viewModel.confirmLogout()
        advanceUntilIdle()

        assertThat(fakeInteractor.logoutCalled).isTrue()
        assertThat(events).containsExactly(ProfileScreenEvents.NavigateToLoginScreen)
        assertThat(viewModel.state.value.isLoggingOut).isFalse()

        job.cancel()
    }

    @Test
    fun `confirmLogout does nothing when already logging out`() = runTest {
        fakeInteractor.logoutDelay = true
        createViewModel()
        advanceUntilIdle()

        viewModel.clickOnLogout()
        viewModel.confirmLogout()
        advanceUntilIdle()

        val initialLogoutCount = fakeInteractor.logoutCallCount

        // Try to confirm again while logging out
        viewModel.confirmLogout()
        advanceUntilIdle()

        assertThat(fakeInteractor.logoutCallCount).isEqualTo(initialLogoutCount)

        fakeInteractor.completeLogout()
        advanceUntilIdle()
    }

    @Test
    fun `onPermissionClicked sets selected permission`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.onPermissionClicked(Permission.ServerAdmin)

        assertThat(viewModel.state.value.selectedPermission).isEqualTo(Permission.ServerAdmin)
    }

    @Test
    fun `onPermissionDialogDismissed clears selected permission`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.onPermissionClicked(Permission.ServerAdmin)
        assertThat(viewModel.state.value.selectedPermission).isNotNull()

        viewModel.onPermissionDialogDismissed()

        assertThat(viewModel.state.value.selectedPermission).isNull()
    }

    private class FakeInteractor : ProfileScreenViewModel.Interactor {
        private var _userName = ""
        private var _baseUrl = ""
        private var _buildVariant = ""
        private var _versionName = ""
        private var _gitCommit = ""

        val serverVersionFlow = MutableStateFlow("disconnected")
        val permissionsFlow = MutableStateFlow<Set<Permission>>(emptySet())

        var logoutCalled = false
        var logoutCallCount = 0
        var logoutDelay = false
        private var logoutContinuation: (() -> Unit)? = null

        fun setUserName(name: String) { _userName = name }
        fun setBaseUrl(url: String) { _baseUrl = url }
        fun setBuildVariant(variant: String) { _buildVariant = variant }
        fun setVersionName(version: String) { _versionName = version }
        fun setGitCommit(commit: String) { _gitCommit = commit }

        override suspend fun logout() {
            logoutCalled = true
            logoutCallCount++

            if (logoutDelay) {
                kotlinx.coroutines.suspendCancellableCoroutine { cont ->
                    logoutContinuation = { cont.resume(Unit) {} }
                }
            }
        }

        fun completeLogout() {
            logoutContinuation?.invoke()
        }

        override fun getUserName(): String = _userName

        override fun getBaseUrl(): String = _baseUrl

        override fun getBuildVariant(): String = _buildVariant

        override fun getVersionName(): String = _versionName

        override fun getGitCommit(): String = _gitCommit

        override fun observeServerVersion(): Flow<String> = serverVersionFlow

        override fun observePermissions(): Flow<Set<Permission>> = permissionsFlow
    }
}

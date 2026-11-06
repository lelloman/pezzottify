package com.lelloman.pezzottify.android.ui.screen.main.devices

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.flowOf
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
class DevicesScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()
    private val playbackErrors = MutableSharedFlow<String>()

    private val interactor = object : DevicesScreenViewModel.Interactor {
        override fun observeDevicesScreenState(): Flow<DevicesScreenState> = flowOf(DevicesScreenState())
        override fun observeRemoteControlDeviceId(): Flow<Int?> = flowOf(null)
        override fun observePlaybackErrorMessages(): Flow<String> = playbackErrors
        override fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int) = Unit
        override fun enterRemoteMode(deviceId: Int, deviceName: String) = Unit
        override fun exitRemoteMode() = Unit
        override suspend fun fetchOwnRegisteredDevices(): List<RegisteredDeviceInfo> = emptyList()
        override suspend fun updateDeviceSharePolicy(
            deviceId: Int,
            state: DeviceSharePolicyUiState,
        ) = DevicesScreenViewModel.DeviceSharePolicyResult.Error("unused")
    }

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `each playback error becomes a message for the snackbar`() = runTest(testDispatcher) {
        val viewModel = DevicesScreenViewModel(interactor)
        val messages = mutableListOf<String>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { viewModel.toastEvents.collect { messages.add(it) } }
        advanceUntilIdle()

        playbackErrors.emit("Not allowed")
        playbackErrors.emit("That device is gone")
        advanceUntilIdle()

        assertThat(messages).containsExactly("Not allowed", "That device is gone").inOrder()
    }
}

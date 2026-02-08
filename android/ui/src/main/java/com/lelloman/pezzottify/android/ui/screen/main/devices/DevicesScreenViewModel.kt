package com.lelloman.pezzottify.android.ui.screen.main.devices

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class DeviceUiState(
    val id: Int,
    val name: String,
    val deviceType: String,
    val isThisDevice: Boolean,
    val trackTitle: String?,
    val artistName: String?,
    val albumImageUrl: String?,
    val isPlaying: Boolean,
    val positionSec: Double,
    val durationMs: Long,
    val timestamp: Long = 0L,
)

data class DevicesScreenState(
    val devices: List<DeviceUiState> = emptyList(),
    val thisDeviceId: Int? = null,
    val remoteControlDeviceId: Int? = null,
)

@HiltViewModel
class DevicesScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(DevicesScreenState())
    val state: StateFlow<DevicesScreenState> = mutableState.asStateFlow()

    private var lastBaseState = DevicesScreenState()
    private var interpolationJob: Job? = null

    init {
        viewModelScope.launch {
            interactor.observeDevicesScreenState().collect { newState ->
                lastBaseState = newState
                mutableState.value = interpolatePositions(newState)
                ensureInterpolation(newState)
            }
        }
        viewModelScope.launch {
            interactor.observeRemoteControlDeviceId().collect { deviceId ->
                mutableState.value = mutableState.value.copy(remoteControlDeviceId = deviceId)
            }
        }
    }

    private fun ensureInterpolation(state: DevicesScreenState) {
        val hasPlayingRemote = state.devices.any { !it.isThisDevice && it.isPlaying && it.trackTitle != null }
        if (hasPlayingRemote && interpolationJob == null) {
            interpolationJob = viewModelScope.launch {
                while (true) {
                    delay(INTERPOLATION_TICK_MS)
                    mutableState.value = interpolatePositions(lastBaseState)
                }
            }
        } else if (!hasPlayingRemote) {
            interpolationJob?.cancel()
            interpolationJob = null
        }
    }

    private fun interpolatePositions(baseState: DevicesScreenState): DevicesScreenState {
        val now = System.currentTimeMillis()
        return baseState.copy(
            devices = baseState.devices.map { device ->
                if (!device.isThisDevice && device.isPlaying && device.trackTitle != null && device.timestamp > 0) {
                    val elapsed = (now - device.timestamp) / 1000.0
                    val interpolated = (device.positionSec + elapsed)
                        .coerceIn(0.0, device.durationMs / 1000.0)
                    device.copy(positionSec = interpolated)
                } else {
                    device
                }
            }
        )
    }

    fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int) {
        interactor.sendCommand(command, payload, targetDeviceId)
    }

    fun seekRemote(targetDeviceId: Int, positionSec: Double) {
        interactor.sendCommand("seek", mapOf("position" to positionSec), targetDeviceId)
    }

    fun enterRemoteMode(deviceId: Int, deviceName: String) {
        interactor.enterRemoteMode(deviceId, deviceName)
    }

    fun exitRemoteMode() {
        interactor.exitRemoteMode()
    }

    interface Interactor {
        fun observeDevicesScreenState(): Flow<DevicesScreenState>
        fun observeRemoteControlDeviceId(): Flow<Int?>
        fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int)
        fun enterRemoteMode(deviceId: Int, deviceName: String)
        fun exitRemoteMode()
    }

    companion object {
        private const val INTERPOLATION_TICK_MS = 200L
    }
}

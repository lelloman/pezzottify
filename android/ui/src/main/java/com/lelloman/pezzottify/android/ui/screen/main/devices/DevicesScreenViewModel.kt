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

data class DeviceSharePolicyUiState(
    val mode: String = "deny_everyone",
    val allowUsers: String = "",
    val denyUsers: String = "",
    val allowAdmin: Boolean = false,
    val allowRegular: Boolean = false,
    val isLoading: Boolean = false,
    val isSaving: Boolean = false,
    val error: String? = null,
)

@HiltViewModel
class DevicesScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(DevicesScreenState())
    val state: StateFlow<DevicesScreenState> = mutableState.asStateFlow()

    private val mutableSharePolicy = MutableStateFlow(DeviceSharePolicyUiState())
    val sharePolicy: StateFlow<DeviceSharePolicyUiState> = mutableSharePolicy.asStateFlow()

    private var lastBaseState = DevicesScreenState()
    private var interpolationJob: Job? = null
    private var lastPolicyDeviceId: Int? = null

    init {
        viewModelScope.launch {
            interactor.observeDevicesScreenState().collect { newState ->
                lastBaseState = newState
                mutableState.value = interpolatePositions(newState)
                ensureInterpolation(newState)
                val deviceId = newState.thisDeviceId
                if (deviceId != null && deviceId != lastPolicyDeviceId) {
                    lastPolicyDeviceId = deviceId
                    loadSharePolicy(deviceId)
                }
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

    fun updatePolicyMode(mode: String) {
        mutableSharePolicy.value = mutableSharePolicy.value.copy(mode = mode, error = null)
    }

    fun updateAllowUsers(text: String) {
        mutableSharePolicy.value = mutableSharePolicy.value.copy(allowUsers = text, error = null)
    }

    fun updateDenyUsers(text: String) {
        mutableSharePolicy.value = mutableSharePolicy.value.copy(denyUsers = text, error = null)
    }

    fun updateAllowAdmin(enabled: Boolean) {
        mutableSharePolicy.value = mutableSharePolicy.value.copy(allowAdmin = enabled, error = null)
    }

    fun updateAllowRegular(enabled: Boolean) {
        mutableSharePolicy.value = mutableSharePolicy.value.copy(allowRegular = enabled, error = null)
    }

    fun saveSharePolicy() {
        val deviceId = lastPolicyDeviceId ?: return
        viewModelScope.launch {
            val current = mutableSharePolicy.value
            mutableSharePolicy.value = current.copy(isSaving = true, error = null)
            val response = interactor.updateDeviceSharePolicy(deviceId, current)
            mutableSharePolicy.value = when (response) {
                is DeviceSharePolicyResult.Success -> mapPolicyToUi(response.policy).copy(isSaving = false)
                is DeviceSharePolicyResult.Error -> current.copy(isSaving = false, error = response.message)
            }
        }
    }

    private fun loadSharePolicy(deviceId: Int) {
        viewModelScope.launch {
            mutableSharePolicy.value = mutableSharePolicy.value.copy(isLoading = true, error = null)
            val response = interactor.fetchDeviceSharePolicy(deviceId)
            mutableSharePolicy.value = when (response) {
                is DeviceSharePolicyResult.Success -> mapPolicyToUi(response.policy).copy(isLoading = false)
                is DeviceSharePolicyResult.Error -> DeviceSharePolicyUiState(isLoading = false, error = response.message)
            }
        }
    }

    private fun mapPolicyToUi(policy: com.lelloman.pezzottify.android.domain.remoteapi.response.DeviceSharePolicy): DeviceSharePolicyUiState {
        val roles = policy.allowRoles.toSet()
        return DeviceSharePolicyUiState(
            mode = policy.mode,
            allowUsers = policy.allowUsers.joinToString(", "),
            denyUsers = policy.denyUsers.joinToString(", "),
            allowAdmin = roles.contains("admin"),
            allowRegular = roles.contains("regular"),
        )
    }

    interface Interactor {
        fun observeDevicesScreenState(): Flow<DevicesScreenState>
        fun observeRemoteControlDeviceId(): Flow<Int?>
        fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int)
        fun enterRemoteMode(deviceId: Int, deviceName: String)
        fun exitRemoteMode()
        suspend fun fetchDeviceSharePolicy(deviceId: Int): DeviceSharePolicyResult
        suspend fun updateDeviceSharePolicy(
            deviceId: Int,
            state: DeviceSharePolicyUiState
        ): DeviceSharePolicyResult
    }

    sealed interface DeviceSharePolicyResult {
        data class Success(val policy: com.lelloman.pezzottify.android.domain.remoteapi.response.DeviceSharePolicy) : DeviceSharePolicyResult
        data class Error(val message: String) : DeviceSharePolicyResult
    }

    companion object {
        private const val INTERPOLATION_TICK_MS = 200L
    }
}

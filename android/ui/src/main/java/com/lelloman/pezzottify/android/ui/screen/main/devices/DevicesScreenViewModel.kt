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
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import javax.inject.Inject

data class DeviceUiState(
    val id: Int,
    val name: String,
    val deviceType: String,
    val isThisDevice: Boolean,
    val isOwnDevice: Boolean = false,
    val isOnline: Boolean = true,
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

data class DeviceSharePolicyUi(
    val mode: String,
    val allowUsers: List<Int>,
    val denyUsers: List<Int>,
    val allowRoles: List<String>,
)

data class RegisteredDeviceInfo(
    val id: Int,
    val name: String,
    val deviceType: String,
    val sharePolicy: DeviceSharePolicyUi,
)

@HiltViewModel
class DevicesScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(DevicesScreenState())
    val state: StateFlow<DevicesScreenState> = mutableState.asStateFlow()

    private val mutableSharePolicies = MutableStateFlow<Map<Int, DeviceSharePolicyUiState>>(emptyMap())
    val sharePolicies: StateFlow<Map<Int, DeviceSharePolicyUiState>> = mutableSharePolicies.asStateFlow()

    private val ownRegisteredDevices = MutableStateFlow<List<RegisteredDeviceInfo>>(emptyList())

    private var lastBaseState = DevicesScreenState()
    private var interpolationJob: Job? = null

    init {
        viewModelScope.launch {
            val registered = interactor.fetchOwnRegisteredDevices()
            ownRegisteredDevices.value = registered
            mutableSharePolicies.value = registered.associate { device ->
                device.id to mapPolicyToUi(device.sharePolicy)
            }
        }
        viewModelScope.launch {
            combine(
                interactor.observeDevicesScreenState(),
                ownRegisteredDevices,
            ) { wsState, registered ->
                mergeDevices(wsState, registered)
            }.collect { newState ->
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

    private fun mergeDevices(
        wsState: DevicesScreenState,
        registered: List<RegisteredDeviceInfo>,
    ): DevicesScreenState {
        val registeredIds = registered.map { it.id }.toSet()
        val wsDeviceIds = wsState.devices.map { it.id }.toSet()

        val taggedWsDevices = wsState.devices.map { device ->
            device.copy(
                isOwnDevice = device.isThisDevice || device.id in registeredIds,
                isOnline = true,
            )
        }

        val offlineDevices = registered
            .filter { it.id !in wsDeviceIds }
            .map { device ->
                DeviceUiState(
                    id = device.id,
                    name = device.name,
                    deviceType = device.deviceType,
                    isThisDevice = false,
                    isOwnDevice = true,
                    isOnline = false,
                    trackTitle = null,
                    artistName = null,
                    albumImageUrl = null,
                    isPlaying = false,
                    positionSec = 0.0,
                    durationMs = 0L,
                )
            }

        return wsState.copy(
            devices = taggedWsDevices + offlineDevices,
        )
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

    fun updatePolicyMode(deviceId: Int, mode: String) {
        updatePolicyField(deviceId) { it.copy(mode = mode, error = null) }
    }

    fun updateAllowUsers(deviceId: Int, text: String) {
        updatePolicyField(deviceId) { it.copy(allowUsers = text, error = null) }
    }

    fun updateDenyUsers(deviceId: Int, text: String) {
        updatePolicyField(deviceId) { it.copy(denyUsers = text, error = null) }
    }

    fun updateAllowAdmin(deviceId: Int, enabled: Boolean) {
        updatePolicyField(deviceId) { it.copy(allowAdmin = enabled, error = null) }
    }

    fun updateAllowRegular(deviceId: Int, enabled: Boolean) {
        updatePolicyField(deviceId) { it.copy(allowRegular = enabled, error = null) }
    }

    fun saveSharePolicy(deviceId: Int) {
        val current = mutableSharePolicies.value[deviceId] ?: return
        viewModelScope.launch {
            updatePolicyField(deviceId) { it.copy(isSaving = true, error = null) }
            val response = interactor.updateDeviceSharePolicy(deviceId, current)
            updatePolicyField(deviceId) { state ->
                when (response) {
                    is DeviceSharePolicyResult.Success -> mapPolicyToUi(response.policy).copy(isSaving = false)
                    is DeviceSharePolicyResult.Error -> state.copy(isSaving = false, error = response.message)
                }
            }
        }
    }

    private fun updatePolicyField(deviceId: Int, transform: (DeviceSharePolicyUiState) -> DeviceSharePolicyUiState) {
        val current = mutableSharePolicies.value[deviceId] ?: DeviceSharePolicyUiState()
        mutableSharePolicies.value = mutableSharePolicies.value + (deviceId to transform(current))
    }

    private fun mapPolicyToUi(policy: DeviceSharePolicyUi): DeviceSharePolicyUiState {
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
        suspend fun fetchOwnRegisteredDevices(): List<RegisteredDeviceInfo>
        suspend fun updateDeviceSharePolicy(
            deviceId: Int,
            state: DeviceSharePolicyUiState
        ): DeviceSharePolicyResult
    }

    sealed interface DeviceSharePolicyResult {
        data class Success(val policy: DeviceSharePolicyUi) : DeviceSharePolicyResult
        data class Error(val message: String) : DeviceSharePolicyResult
    }

    companion object {
        private const val INTERPOLATION_TICK_MS = 200L
    }
}

package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class DevicesResponse(
    val devices: List<DeviceInfo>,
)

@Serializable
data class DeviceInfo(
    val id: Int,
    @SerialName("device_uuid")
    val deviceUuid: String,
    @SerialName("device_type")
    val deviceType: String,
    @SerialName("device_name")
    val deviceName: String? = null,
    @SerialName("os_info")
    val osInfo: String? = null,
    @SerialName("first_seen")
    val firstSeen: Long,
    @SerialName("last_seen")
    val lastSeen: Long,
    @SerialName("share_policy")
    val sharePolicy: DeviceSharePolicy,
)

@Serializable
data class DeviceSharePolicy(
    val mode: String,
    @SerialName("allow_users")
    val allowUsers: List<Int> = emptyList(),
    @SerialName("allow_roles")
    val allowRoles: List<String> = emptyList(),
    @SerialName("deny_users")
    val denyUsers: List<Int> = emptyList(),
)

package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class LoginRequest(
    @SerialName("user_handle")
    val userHandle: String,
    val password: String,
    @SerialName("device_uuid")
    val deviceUuid: String,
    @SerialName("device_type")
    val deviceType: String,
    @SerialName("device_name")
    val deviceName: String?,
    @SerialName("os_info")
    val osInfo: String?,
)
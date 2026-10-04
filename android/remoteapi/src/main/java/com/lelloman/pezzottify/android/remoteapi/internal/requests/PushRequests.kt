package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class PushVapidKeyResponse(
    @SerialName("public_key") val publicKey: String,
)

@Serializable
internal data class PushRegistrationRequest(
    val endpoint: String,
    val p256dh: String,
    val auth: String,
    @SerialName("device_id") val deviceId: String? = null,
)

@Serializable
internal data class PushRegistrationDeleteRequest(
    val endpoint: String,
)

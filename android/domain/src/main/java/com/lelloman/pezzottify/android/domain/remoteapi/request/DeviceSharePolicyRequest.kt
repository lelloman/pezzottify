package com.lelloman.pezzottify.android.domain.remoteapi.request

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class DeviceSharePolicyRequest(
    val mode: String,
    @SerialName("allow_users")
    val allowUsers: List<Int> = emptyList(),
    @SerialName("allow_roles")
    val allowRoles: List<String> = emptyList(),
    @SerialName("deny_users")
    val denyUsers: List<Int> = emptyList(),
)

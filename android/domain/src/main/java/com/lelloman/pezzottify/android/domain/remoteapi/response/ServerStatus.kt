package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Public server status returned by `GET /v1/status`. */
@Serializable
data class ServerStatus(
    val enabled: Boolean = false,
    val message: String? = null,
    @SerialName("estimated_end") val estimatedEnd: String? = null,
)

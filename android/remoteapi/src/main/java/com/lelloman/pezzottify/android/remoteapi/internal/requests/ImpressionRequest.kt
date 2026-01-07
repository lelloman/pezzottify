package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class ImpressionRequest(
    @SerialName("item_type") val itemType: String,
    @SerialName("item_id") val itemId: String,
)

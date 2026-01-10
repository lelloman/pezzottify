package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class SearchRequest(
    val query: String,
    val filters: List<String>?,
    @SerialName("exclude_unavailable")
    val excludeUnavailable: Boolean = false,
    val resolve: Boolean = false,
)
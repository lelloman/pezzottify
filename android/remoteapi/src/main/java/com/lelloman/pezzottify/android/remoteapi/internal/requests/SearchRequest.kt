package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.Serializable

@Serializable
data class SearchRequest(
    val query: String,
    val filters: List<String>?,
)
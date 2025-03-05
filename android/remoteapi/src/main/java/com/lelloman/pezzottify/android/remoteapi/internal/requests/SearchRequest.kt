package com.lelloman.pezzottify.android.remoteapi.internal.requests

data class SearchRequest(
    val query: String,
    val filters: List<String>,
)
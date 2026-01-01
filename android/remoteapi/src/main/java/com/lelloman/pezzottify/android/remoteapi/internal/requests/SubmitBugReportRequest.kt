package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.Serializable

@Serializable
internal data class SubmitBugReportRequest(
    val title: String?,
    val description: String,
    val clientType: String,
    val clientVersion: String?,
    val deviceInfo: String?,
    val logs: String?,
    val attachments: List<String>?,
)

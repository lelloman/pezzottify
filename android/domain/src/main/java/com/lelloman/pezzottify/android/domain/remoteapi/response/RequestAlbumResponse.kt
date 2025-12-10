package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Response for POST /v1/download/request/album.
 */
@Serializable
data class RequestAlbumResponse(
    /** ID of the created queue item */
    @SerialName("request_id")
    val requestId: String,
    /** Initial status (usually PENDING) */
    val status: DownloadQueueStatus,
    /** Position in the queue */
    @SerialName("queue_position")
    val queuePosition: Int,
)

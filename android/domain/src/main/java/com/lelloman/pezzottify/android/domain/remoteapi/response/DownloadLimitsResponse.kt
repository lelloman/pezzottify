package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * User's rate limit status for download requests.
 */
@Serializable
data class DownloadLimitsResponse(
    /** Number of requests made today */
    @SerialName("requests_today")
    val requestsToday: Int,
    /** Maximum requests allowed per day */
    @SerialName("max_per_day")
    val maxPerDay: Int,
    /** Number of items currently in queue for this user */
    @SerialName("in_queue")
    val inQueue: Int,
    /** Maximum items allowed in queue */
    @SerialName("max_queue")
    val maxQueue: Int,
    /** Whether the user can make more requests */
    @SerialName("can_request")
    val canRequest: Boolean,
)

package com.lelloman.pezzottify.android.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ArtistResponse(
    val id: String,

    val name: String,

    val genre: List<String>?,

    val portraits: List<Image>?,

    val related: List<String>?,

    @SerialName("portrait_group")
    val portraitGroup: List<Image>?,

    @SerialName("activity_periods")
    val activityPeriods: List<ActivityPeriod>?,
)
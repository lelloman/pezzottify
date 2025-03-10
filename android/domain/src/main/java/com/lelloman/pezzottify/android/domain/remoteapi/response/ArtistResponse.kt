package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Artist
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

fun ArtistResponse.toDomain() = object : Artist {
    override val id: String
        get() = this@toDomain.id
    override val name: String
        get() = this@toDomain.name
}
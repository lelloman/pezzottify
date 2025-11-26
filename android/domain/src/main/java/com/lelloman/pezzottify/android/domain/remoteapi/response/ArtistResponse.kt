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
    override val portraits: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = this@toDomain.portraits?.map { it.toDomain() } ?: emptyList()
    override val portraitGroup: List<com.lelloman.pezzottify.android.domain.statics.Image>
        get() = this@toDomain.portraitGroup?.map { it.toDomain() } ?: emptyList()
    override val related: List<String>
        get() = this@toDomain.related ?: emptyList()
}

private fun Image.toDomain() = com.lelloman.pezzottify.android.domain.statics.Image(
    id = id,
    size = when (size) {
        ImageSize.DEFAULT -> com.lelloman.pezzottify.android.domain.statics.ImageSize.DEFAULT
        ImageSize.SMALL -> com.lelloman.pezzottify.android.domain.statics.ImageSize.SMALL
        ImageSize.MEDIUM -> com.lelloman.pezzottify.android.domain.statics.ImageSize.MEDIUM
        ImageSize.LARGE -> com.lelloman.pezzottify.android.domain.statics.ImageSize.LARGE
        ImageSize.XLARGE -> com.lelloman.pezzottify.android.domain.statics.ImageSize.XLARGE
    }
)
package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.statics.Track
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class ArtistRole {
    ARTIST_ROLE_UNKNOWN,
    ARTIST_ROLE_MAIN_ARTIST,
    ARTIST_ROLE_FEATURED_ARTIST,
    ARTIST_ROLE_REMIXER,
    ARTIST_ROLE_ACTOR,
    ARTIST_ROLE_COMPOSER,
    ARTIST_ROLE_CONDUCTOR,
    ARTIST_ROLE_ORCHESTRA,
}

@Serializable
data class ArtistWithRole(
    val artistId: String,
    val name: String,
    val role: ArtistRole,
)

@Serializable
data class TrackResponse(

    val id: String,

    val name: String,

    @SerialName("album_id")
    val albumId: String,

    @SerialName("artists_ids")
    val artistsIds: List<String>,

    val number: Int,

    @SerialName("disc_number")
    val discNumber: Int,

    @SerialName("duration")
    val durationMillis: Long,

    @SerialName("is_explicit")
    val isExplicit: Boolean,

    val alternatives: List<String>,

    val tags: List<String>,

    @SerialName("has_lyrics")
    val hasLyrics: Boolean,

    @SerialName("language_of_performance")
    val languageOfPerformance: List<String>,

    @SerialName("original_title")
    val originalTitle: String,

    @SerialName("version_title")
    val versionTitle: String,

    @SerialName("artists_with_role")
    val artistsWithRole: List<ArtistWithRole>,
)

fun TrackResponse.toDomain() = object : Track {
    override val id: String
        get() = this@toDomain.id
    override val name: String
        get() = this@toDomain.name
    override val albumId: String
        get() = this@toDomain.albumId
    override val artistsIds: List<String>
        get() = this@toDomain.artistsIds
    override val durationSeconds: Int = (this@toDomain.durationMillis / 1000L).toInt()
}
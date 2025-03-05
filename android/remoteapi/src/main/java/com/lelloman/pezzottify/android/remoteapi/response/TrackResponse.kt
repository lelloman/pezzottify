package com.lelloman.pezzottify.android.remoteapi.response

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

    val duration: Long,

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
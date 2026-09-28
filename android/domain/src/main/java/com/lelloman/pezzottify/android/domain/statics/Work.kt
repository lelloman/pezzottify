package com.lelloman.pezzottify.android.domain.statics

import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Work(
    val id: String,
    val title: String,
    val creators: List<String> = emptyList(),
    val kind: String? = null,
    @SerialName("catalog_number") val catalogNumber: String? = null,
    @SerialName("musicbrainz_id") val musicbrainzId: String? = null,
    @SerialName("wikidata_id") val wikidataId: String? = null,
    @SerialName("creator_artist_ids") val creatorArtistIds: List<String> = emptyList(),
    @SerialName("composition_year") val compositionYear: String? = null,
)

@Serializable
data class WorkResolution(
    val work: Work? = null,
    val status: String,
    val reason: String = "",
)

@Serializable
data class WorkRelation(
    val work: Work,
    @SerialName("relationship_type") val relationshipType: String,
    val direction: String,
    val ordering: Long = 0,
)

@Serializable
data class RecordingWork(val id: String, val title: String)

@Serializable
data class WorkPage(
    val work: Work,
    val relations: List<WorkRelation> = emptyList(),
    val tracks: List<TrackResponse> = emptyList(),
    @SerialName("next_offset") val nextOffset: Int,
    @SerialName("has_more") val hasMore: Boolean,
)

enum class WorkScope(val value: String) { All("all"), Parts("parts"), Related("related") }

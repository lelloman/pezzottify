package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** `GET /v1/content/concepts`: named musical ideas usable as steering references. */
@Serializable
data class ConceptsResponse(
    val concepts: List<Concept> = emptyList(),
)

@Serializable
data class Concept(
    val id: String,
    val family: String,
    val label: String,
    @SerialName("example_count") val exampleCount: Int = 0,
) {
    companion object {
        const val FAMILY_SOUND_GENRE = "sound_genre"
        const val FAMILY_INSTRUMENT = "instrument"
        const val FAMILY_VOCALS = "vocals"
        const val FAMILY_MOOD = "mood"
        const val FAMILY_GENRE_TAG = "genre_tag"
        const val FAMILY_RECORDED = "recorded"
        const val FAMILY_COMPOSED = "composed"

        /** Display order of families. */
        val FAMILY_ORDER = listOf(
            FAMILY_SOUND_GENRE,
            FAMILY_INSTRUMENT,
            FAMILY_VOCALS,
            FAMILY_MOOD,
            FAMILY_GENRE_TAG,
            FAMILY_RECORDED,
            FAMILY_COMPOSED,
        )
    }
}

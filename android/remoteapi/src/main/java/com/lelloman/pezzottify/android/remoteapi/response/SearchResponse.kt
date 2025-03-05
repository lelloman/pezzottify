package com.lelloman.pezzottify.android.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class HashedItemType {
    Track,
    Artist,
    Album,
}

@Serializable
data class SearchResult(
    @SerialName("item_type")
    val itemType: HashedItemType,

    @SerialName("item_id")
    val itemId: String,

    val score: Long,

    @SerialName("adjusted_score")
    val adjustedScore: Long,

    @SerialName("matchable_text")
    val matchableText: String,
)

typealias SearchResponse = List<SearchResult>
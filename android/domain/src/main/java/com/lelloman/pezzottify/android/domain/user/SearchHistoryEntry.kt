package com.lelloman.pezzottify.android.domain.user

data class SearchHistoryEntry(
    val query: String,
    val contentType: Type,
    val contentId: String,
    val created: Long,
) {
    enum class Type {
        Album,
        Track,
        Artist,
    }
}

package com.lelloman.pezzottify.android.domain.user

interface ViewedContent {
    val type: Type

    val contentId: String

    val created: Long

    val synced: Boolean

    enum class Type {
        Album,
        Track,
        Artist,
        UserPlaylist,
    }
}
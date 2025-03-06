package com.lelloman.pezzottify.android.localdata.statics.model

enum class ErrorReason {
    Network,
    NotFound,
    Client,
    Unknown,
}

sealed interface StaticItemFetchState {
    val itemId: String

    data class Requested(override val itemId: String) : StaticItemFetchState

    data class Loading(override val itemId: String) : StaticItemFetchState

    data class Error(override val itemId: String, val reason: ErrorReason) : StaticItemFetchState
}
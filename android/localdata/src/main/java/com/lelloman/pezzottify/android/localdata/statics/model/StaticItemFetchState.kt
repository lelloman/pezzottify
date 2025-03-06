package com.lelloman.pezzottify.android.localdata.statics.model

enum class ErrorReason {
    Network,
    NotFound,
    Client,
    Unknown;

    companion object {
        fun fromString(string: String) = when (string.lowercase()) {
            "network" -> Network
            "notfound" -> NotFound
            "client" -> Client
            else -> Unknown
        }
    }
}

sealed interface StaticItemFetchState {
    val itemId: String

    data class Requested(override val itemId: String) : StaticItemFetchState

    data class Loading(override val itemId: String) : StaticItemFetchState

    data class Error(override val itemId: String, val reason: ErrorReason) : StaticItemFetchState
}

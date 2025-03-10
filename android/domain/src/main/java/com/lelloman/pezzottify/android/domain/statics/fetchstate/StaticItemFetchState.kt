package com.lelloman.pezzottify.android.domain.statics.fetchstate

import com.lelloman.pezzottify.android.domain.statics.StaticItemType

data class StaticItemFetchState(
    val itemId: String,
    val itemType: StaticItemType,
    val isLoading: Boolean,
    val errorReason: ErrorReason?,
) {
    companion object {
        fun requested(itemId: String, itemType: StaticItemType) = StaticItemFetchState(
            itemId = itemId,
            itemType = itemType,
            isLoading = false,
            errorReason = null,
        )

        fun loading(itemId: String, itemType: StaticItemType) = StaticItemFetchState(
            itemId = itemId,
            itemType = itemType,
            isLoading = true,
            errorReason = null,
        )

        fun error(itemId: String, itemType: StaticItemType) = StaticItemFetchState(
            itemId = itemId,
            itemType = itemType,
            isLoading = false,
            errorReason = ErrorReason.Unknown,
        )
    }
}

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
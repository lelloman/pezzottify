package com.lelloman.pezzottify.android.domain.impression

import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

data class Impression(
    val id: Long = 0,
    val itemId: String,
    val itemType: ItemType,
    val syncStatus: SyncStatus = SyncStatus.PendingSync,
    val createdAt: Long,
)

enum class ItemType {
    Artist,
    Album,
    Track;

    companion object {
        fun fromString(value: String): ItemType = when (value.lowercase()) {
            "artist" -> Artist
            "album" -> Album
            "track" -> Track
            else -> throw IllegalArgumentException("Unknown item type: $value")
        }
    }
}

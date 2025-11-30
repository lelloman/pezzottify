package com.lelloman.pezzottify.android.localdata.internal.user.model

import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry

@Entity(tableName = SearchHistoryEntryEntity.TABLE_NAME)
internal data class SearchHistoryEntryEntity(
    @PrimaryKey
    val id: String,
    val query: String,
    val contentType: SearchHistoryEntry.Type,
    val contentId: String,
    val created: Long,
) {
    companion object {
        const val TABLE_NAME = "search_history_entry"
    }
}

internal fun SearchHistoryEntryEntity.toDomain() = SearchHistoryEntry(
    query = query,
    contentType = contentType,
    contentId = contentId,
    created = created,
)

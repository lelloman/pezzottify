package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.TypeConverter
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.ViewedContent

internal class UserDataTypeConverters {

    @TypeConverter
    fun fromViewedContentType(type: ViewedContent.Type): String = type.name

    @TypeConverter
    fun toViewedContentType(value: String): ViewedContent.Type =
        ViewedContent.Type.valueOf(value)

    @TypeConverter
    fun fromSearchHistoryEntryType(type: SearchHistoryEntry.Type): String = type.name

    @TypeConverter
    fun toSearchHistoryEntryType(value: String): SearchHistoryEntry.Type =
        SearchHistoryEntry.Type.valueOf(value)
}

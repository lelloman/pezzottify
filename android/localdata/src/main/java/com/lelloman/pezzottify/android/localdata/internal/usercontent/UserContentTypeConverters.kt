package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.TypeConverter
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

internal class UserContentTypeConverters {

    private val json = Json { ignoreUnknownKeys = true }

    @TypeConverter
    fun fromContentType(contentType: LikedContent.ContentType): String = contentType.name

    @TypeConverter
    fun toContentType(value: String): LikedContent.ContentType =
        LikedContent.ContentType.valueOf(value)

    @TypeConverter
    fun fromSyncStatus(syncStatus: SyncStatus): String = syncStatus.name

    @TypeConverter
    fun toSyncStatus(value: String): SyncStatus = SyncStatus.valueOf(value)

    @TypeConverter
    fun fromStringList(list: List<String>): String = json.encodeToString(list)

    @TypeConverter
    fun toStringList(value: String): List<String> = json.decodeFromString(value)
}

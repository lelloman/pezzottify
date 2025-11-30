package com.lelloman.pezzottify.android.localdata.internal.usercontent.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

@Entity(tableName = LikedContentEntity.TABLE_NAME)
internal data class LikedContentEntity(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_CONTENT_ID)
    override val contentId: String,

    @ColumnInfo(name = COLUMN_CONTENT_TYPE)
    override val contentType: LikedContent.ContentType,

    @ColumnInfo(name = COLUMN_IS_LIKED)
    override val isLiked: Boolean,

    @ColumnInfo(name = COLUMN_MODIFIED_AT)
    override val modifiedAt: Long,

    @ColumnInfo(name = COLUMN_SYNC_STATUS)
    override val syncStatus: SyncStatus,
) : LikedContent {
    companion object {
        const val TABLE_NAME = "liked_content"

        const val COLUMN_CONTENT_ID = "content_id"
        const val COLUMN_CONTENT_TYPE = "content_type"
        const val COLUMN_IS_LIKED = "is_liked"
        const val COLUMN_MODIFIED_AT = "modified_at"
        const val COLUMN_SYNC_STATUS = "sync_status"
    }
}

internal fun LikedContent.toEntity() = LikedContentEntity(
    contentId = contentId,
    contentType = contentType,
    isLiked = isLiked,
    modifiedAt = modifiedAt,
    syncStatus = syncStatus,
)

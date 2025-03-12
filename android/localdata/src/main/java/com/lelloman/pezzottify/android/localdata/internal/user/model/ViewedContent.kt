package com.lelloman.pezzottify.android.localdata.internal.user.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

private typealias DomainViewedContent = com.lelloman.pezzottify.android.domain.user.ViewedContent
private typealias DomainViewedContentType = com.lelloman.pezzottify.android.domain.user.ViewedContent.Type

@Entity(tableName = ViewedContent.TABLE_NAME)
internal data class ViewedContent(
    @PrimaryKey(autoGenerate = true)
    val id: Long,

    @ColumnInfo(name = ViewedContent.COLUMN_TYPE)
    override val type: DomainViewedContentType,

    @ColumnInfo(name = ViewedContent.COLUMN_CONTENT_ID)
    override val contentId: String,

    @ColumnInfo(name = ViewedContent.COLUMN_CREATED)
    override val created: Long,
    override val synced: Boolean,
) : DomainViewedContent {
    companion object {
        const val TABLE_NAME = "viewed_content"

        const val COLUMN_TYPE = "type"
        const val COLUMN_CONTENT_ID = "contentId"
        const val COLUMN_CREATED = "created"
    }
}

internal fun DomainViewedContent.toLocalData() = ViewedContent(
    id = 0,
    type = type,
    contentId = contentId,
    created = created,
    synced = synced,
)

val DomainViewedContentType.dbValue: String get() = this.name
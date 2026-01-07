package com.lelloman.pezzottify.android.localdata.internal.impression

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "impression",
    indices = [
        Index("sync_status"),
    ]
)
internal data class ImpressionEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "item_id") val itemId: String,
    @ColumnInfo(name = "item_type") val itemType: String,
    @ColumnInfo(name = "sync_status") val syncStatus: String,
    @ColumnInfo(name = "created_at") val createdAt: Long,
)

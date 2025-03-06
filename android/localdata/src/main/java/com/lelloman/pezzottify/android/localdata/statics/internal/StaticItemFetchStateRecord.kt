package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.localdata.statics.model.ErrorReason
import com.lelloman.pezzottify.android.localdata.statics.model.StaticItemFetchState

@Entity(tableName = StaticItemFetchStateRecord.TABLE_NAME)
internal data class StaticItemFetchStateRecord(

    @PrimaryKey
    @ColumnInfo(name = COLUMN_ITEM_ID)
    val itemId: String,

    val loading: Boolean,

    val errorReason: String?,

    val lastUpdated: Long,

    val created: Long,
) {
    companion object {
        const val TABLE_NAME = "static_item_fetch_state"

        const val COLUMN_ITEM_ID = "item_id"

        fun StaticItemFetchStateRecord.toState() = when {
            this.loading -> StaticItemFetchState.Loading(this.itemId)
            this.errorReason != null -> StaticItemFetchState.Error(
                this.itemId,
                ErrorReason.fromString(errorReason)
            )

            else -> StaticItemFetchState.Requested(this.itemId)
        }
    }
}
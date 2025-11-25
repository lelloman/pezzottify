package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.statics.StaticItemType
import com.lelloman.pezzottify.android.domain.statics.fetchstate.ErrorReason
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchState

@Entity(tableName = StaticItemFetchStateRecord.TABLE_NAME)
internal data class StaticItemFetchStateRecord(

    @PrimaryKey
    @ColumnInfo(name = COLUMN_ITEM_ID)
    val itemId: String,

    val loading: Boolean,

    val errorReason: String?,

    val itemType: StaticItemType,

    val lastAttemptTime: Long?,

    val tryNextTime: Long?,

    ) {
    companion object {
        const val TABLE_NAME = "static_item_fetch_state"

        const val COLUMN_ITEM_ID = "item_id"
        const val COLUMN_LOADING = "loading"
        const val COLUMN_TRY_NEXT_TIME = "tryNextTime"

        fun StaticItemFetchState.toRecord() = StaticItemFetchStateRecord(
            itemId = itemId,
            loading = isLoading,
            itemType = itemType,
            errorReason = errorReason?.toString(),
            lastAttemptTime = lastAttemptTime,
            tryNextTime = tryNextTime,
        )

        fun StaticItemFetchStateRecord.toDomain() = StaticItemFetchState(
            itemId = itemId,
            isLoading = loading,
            itemType = itemType,
            errorReason = errorReason?.let {
                try {
                    ErrorReason.fromString(it)
                } catch (_: Throwable) {
                    ErrorReason.Unknown
                }
            },
            lastAttemptTime = lastAttemptTime,
            tryNextTime = tryNextTime,
        )
    }
}
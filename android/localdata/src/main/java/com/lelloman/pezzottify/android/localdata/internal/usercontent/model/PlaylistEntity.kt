package com.lelloman.pezzottify.android.localdata.internal.usercontent.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylist

@Entity(tableName = PlaylistEntity.TABLE_NAME)
internal data class PlaylistEntity(
    @PrimaryKey
    @ColumnInfo(name = COLUMN_ID)
    val id: String,

    @ColumnInfo(name = COLUMN_NAME)
    val name: String,

    @ColumnInfo(name = COLUMN_TRACK_IDS)
    val trackIds: List<String>,

    @ColumnInfo(name = COLUMN_SYNC_STATUS, defaultValue = "Synced")
    val syncStatus: PlaylistSyncStatus = PlaylistSyncStatus.Synced,
) {
    companion object {
        const val TABLE_NAME = "playlist"

        const val COLUMN_ID = "id"
        const val COLUMN_NAME = "name"
        const val COLUMN_TRACK_IDS = "track_ids"
        const val COLUMN_SYNC_STATUS = "sync_status"
    }
}

internal fun PlaylistEntity.toDomain(): UserPlaylist = object : UserPlaylist {
    override val id = this@toDomain.id
    override val name = this@toDomain.name
    override val trackIds = this@toDomain.trackIds
    override val syncStatus = this@toDomain.syncStatus
}

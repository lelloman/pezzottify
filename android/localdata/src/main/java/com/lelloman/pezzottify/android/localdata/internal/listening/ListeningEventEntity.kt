package com.lelloman.pezzottify.android.localdata.internal.listening

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "listening_event",
    indices = [
        Index("sync_status"),
        Index("session_id", unique = true),
    ]
)
internal data class ListeningEventEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "track_id") val trackId: String,
    @ColumnInfo(name = "session_id") val sessionId: String,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "ended_at") val endedAt: Long?,
    @ColumnInfo(name = "duration_seconds") val durationSeconds: Int,
    @ColumnInfo(name = "track_duration_seconds") val trackDurationSeconds: Int,
    @ColumnInfo(name = "seek_count") val seekCount: Int,
    @ColumnInfo(name = "pause_count") val pauseCount: Int,
    @ColumnInfo(name = "playback_context") val playbackContext: String,
    @ColumnInfo(name = "sync_status") val syncStatus: String,
    @ColumnInfo(name = "created_at") val createdAt: Long,
)

package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventDao
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventEntity
import com.lelloman.pezzottify.android.localdata.internal.notifications.NotificationDao
import com.lelloman.pezzottify.android.localdata.internal.notifications.NotificationEntity
import com.lelloman.pezzottify.android.localdata.internal.notifications.PendingNotificationReadEntity
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.PlaylistEntity

@Database(
    entities = [
        LikedContentEntity::class,
        ListeningEventEntity::class,
        PlaylistEntity::class,
        NotificationEntity::class,
        PendingNotificationReadEntity::class,
    ],
    version = UserContentDb.VERSION,
    exportSchema = false,
)
@TypeConverters(UserContentTypeConverters::class)
internal abstract class UserContentDb : RoomDatabase() {

    abstract fun likedContentDao(): LikedContentDao

    abstract fun listeningEventDao(): ListeningEventDao

    abstract fun playlistDao(): PlaylistDao

    abstract fun notificationDao(): NotificationDao

    companion object {
        const val VERSION = 5
        const val NAME = "user_content"

        val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS listening_event (
                        id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                        track_id TEXT NOT NULL,
                        session_id TEXT NOT NULL,
                        started_at INTEGER NOT NULL,
                        ended_at INTEGER,
                        duration_seconds INTEGER NOT NULL,
                        track_duration_seconds INTEGER NOT NULL,
                        seek_count INTEGER NOT NULL,
                        pause_count INTEGER NOT NULL,
                        playback_context TEXT NOT NULL,
                        sync_status TEXT NOT NULL,
                        created_at INTEGER NOT NULL
                    )
                    """.trimIndent()
                )
                db.execSQL("CREATE INDEX IF NOT EXISTS index_listening_event_sync_status ON listening_event (sync_status)")
                db.execSQL("CREATE UNIQUE INDEX IF NOT EXISTS index_listening_event_session_id ON listening_event (session_id)")
            }
        }

        val MIGRATION_2_3 = object : Migration(2, 3) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS playlist (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL,
                        track_ids TEXT NOT NULL
                    )
                    """.trimIndent()
                )
            }
        }

        val MIGRATION_3_4 = object : Migration(3, 4) {
            override fun migrate(db: SupportSQLiteDatabase) {
                // Create notification table
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS notification (
                        id TEXT PRIMARY KEY NOT NULL,
                        notification_type TEXT NOT NULL,
                        title TEXT NOT NULL,
                        body TEXT,
                        data TEXT NOT NULL,
                        read_at INTEGER,
                        created_at INTEGER NOT NULL
                    )
                    """.trimIndent()
                )
                db.execSQL("CREATE INDEX IF NOT EXISTS index_notification_created_at ON notification (created_at DESC)")

                // Create pending notification read table for offline queue
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS pending_notification_read (
                        notification_id TEXT PRIMARY KEY NOT NULL,
                        read_at INTEGER NOT NULL,
                        created_at INTEGER NOT NULL,
                        retry_count INTEGER NOT NULL DEFAULT 0
                    )
                    """.trimIndent()
                )
            }
        }

        val MIGRATION_4_5 = object : Migration(4, 5) {
            override fun migrate(db: SupportSQLiteDatabase) {
                // Add sync_status column to playlist table with default 'Synced'
                db.execSQL(
                    """
                    ALTER TABLE playlist ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'Synced'
                    """.trimIndent()
                )
                db.execSQL("CREATE INDEX IF NOT EXISTS index_playlist_sync_status ON playlist (sync_status)")
            }
        }
    }
}

package com.lelloman.pezzottify.android.localdata.internal.usercontent

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventDao
import com.lelloman.pezzottify.android.localdata.internal.listening.ListeningEventEntity
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity

@Database(
    entities = [LikedContentEntity::class, ListeningEventEntity::class],
    version = UserContentDb.VERSION,
)
@TypeConverters(UserContentTypeConverters::class)
internal abstract class UserContentDb : RoomDatabase() {

    abstract fun likedContentDao(): LikedContentDao

    abstract fun listeningEventDao(): ListeningEventDao

    companion object {
        const val VERSION = 2
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
    }
}

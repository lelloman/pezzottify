package com.lelloman.pezzottify.android.localdata.internal.statics

import android.content.Context
import androidx.room.Room
import androidx.sqlite.db.SupportSQLiteDatabase
import androidx.sqlite.db.SupportSQLiteOpenHelper
import androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.statics.Work
import com.lelloman.pezzottify.android.domain.statics.WorkResolution
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Track
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class WorkCacheTest {
    @Test fun `migration adds nullable work fields and preserves old track`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val helper = FrameworkSQLiteOpenHelperFactory().create(SupportSQLiteOpenHelper.Configuration.builder(context)
            .callback(object : SupportSQLiteOpenHelper.Callback(12) {
                override fun onCreate(db: SupportSQLiteDatabase) {
                    db.execSQL("CREATE TABLE Track (id TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL)")
                    db.execSQL("INSERT INTO Track VALUES ('t', 'Old recording')")
                }
                override fun onUpgrade(db: SupportSQLiteDatabase, oldVersion: Int, newVersion: Int) = Unit
            }).build())
        helper.use {
            val db = helper.writableDatabase
            StaticsDb.MIGRATION_12_13.migrate(db)
            db.query("SELECT name, work_resolution, work_enrichment_status FROM Track WHERE id='t'").use { cursor ->
                assertThat(cursor.moveToFirst()).isTrue()
                assertThat(cursor.getString(0)).isEqualTo("Old recording")
                assertThat(cursor.isNull(1)).isTrue()
                assertThat(cursor.isNull(2)).isTrue()
            }
        }
    }

    @Test fun `work link survives Room roundtrip without track enrichment`() = runTest {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, StaticsDb::class.java).allowMainThreadQueries().build()
        try {
            val resolution = WorkResolution(Work("w", "Composition"), "resolved")
            db.staticsDao().insertTrack(Track("t", "Recording", "a", emptyList(), 10, workResolution = resolution))
            val restored = db.staticsDao().getTrack("t").first()!!
            assertThat(restored.enrichment).isNull()
            assertThat(restored.workResolution).isEqualTo(resolution)
        } finally { db.close() }
    }
}

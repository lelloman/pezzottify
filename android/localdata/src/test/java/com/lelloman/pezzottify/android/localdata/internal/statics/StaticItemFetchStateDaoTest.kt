package com.lelloman.pezzottify.android.localdata.internal.statics

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.statics.StaticItemType
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class StaticItemFetchStateDaoTest {

    private val db = Room.inMemoryDatabaseBuilder(
        ApplicationProvider.getApplicationContext<Context>(),
        StaticsDb::class.java,
    ).allowMainThreadQueries().build()
    private val dao = db.staticItemFetchStateDao()

    @After
    fun tearDown() = db.close()

    private fun record(id: String, loading: Boolean, tryNextTime: Long?) = StaticItemFetchStateRecord(
        itemId = id,
        loading = loading,
        errorReason = if (tryNextTime != null) "Unknown" else null,
        itemType = StaticItemType.Track,
        lastAttemptTime = 0L,
        tryNextTime = tryNextTime,
    )

    @Test
    fun `next retry time is the earliest scheduled retry among non-loading items`() = runTest {
        assertThat(dao.getNextRetryTime()).isNull()

        dao.insert(record("requested", loading = false, tryNextTime = null))
        assertThat(dao.getNextRetryTime()).isNull()

        dao.insert(record("later", loading = false, tryNextTime = 5_000L))
        dao.insert(record("sooner", loading = false, tryNextTime = 2_000L))
        dao.insert(record("loading", loading = true, tryNextTime = 1_000L))

        assertThat(dao.getNextRetryTime()).isEqualTo(2_000L)
    }
}

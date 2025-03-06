package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.Room
import androidx.test.platform.app.InstrumentationRegistry
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runners.model.TestTimedOutException
import java.util.concurrent.TimeUnit
import kotlin.random.Random

@OptIn(ExperimentalCoroutinesApi::class)
class StaticsDbTest {

    private val context by lazy { InstrumentationRegistry.getInstrumentation().targetContext }

    private val db by lazy { Room.inMemoryDatabaseBuilder(context, StaticsDb::class.java).build() }

    private val staticsDao by lazy { db.staticsDao() }
    private val staticItemFetchStateDao by lazy { db.staticItemFetchStateDao() }

    private suspend fun List<*>.awaitSize(size: Int) {
        val start = System.currentTimeMillis()
        while (this.size < size) {
            val elapsed = System.currentTimeMillis() - start
            if (elapsed > 1000) {
                throw TestTimedOutException(elapsed, TimeUnit.MILLISECONDS)
            }
            delay(10)
        }
    }

    @Test
    fun handlesArtists() = runTest {
        val artist1 = randomArtist()
        val artist1Id = artist1.id

        val values = mutableListOf<Artist?>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getArtist(artist1Id).toList(values)
        }

        advanceUntilIdle()
        values.awaitSize(1)
        assertThat(values).isEqualTo(listOf(null))

        val artist1InsertResult = staticsDao.insertArtist(artist1)
        assertThat(artist1InsertResult).isEqualTo(1)

        values.awaitSize(2)
        assertThat(values).isEqualTo(listOf(null, artist1))
    }

    private fun randomArtist() = Artist(
        id = Random.nextLong().toString(),
        name = Random.nextLong().toString(),
    )
}
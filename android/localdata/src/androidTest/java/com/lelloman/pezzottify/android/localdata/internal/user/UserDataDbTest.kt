package com.lelloman.pezzottify.android.localdata.internal.user

import androidx.room.Room
import androidx.test.platform.app.InstrumentationRegistry
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.localdata.internal.user.model.dbValue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.*
import org.junit.runners.model.TestTimedOutException
import java.util.concurrent.TimeUnit

private typealias ViewedContentEntity = com.lelloman.pezzottify.android.localdata.internal.user.model.ViewedContent

class UserDataDbTest {

    private val context by lazy { InstrumentationRegistry.getInstrumentation().targetContext }

    private val db by lazy { Room.inMemoryDatabaseBuilder(context, UserDataDb::class.java).build() }

    private val viewedContentDao by lazy { db.viewedContentDao() }

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

    @Before
    fun setUp() {
        runBlocking(Dispatchers.IO) {
            db.clearAllTables()
        }
    }

    @After
    fun tearDown() {
        db.close()
    }

    @Test
    fun handlesViewedContent() = runTest {

        val mostRecent = mutableListOf<List<ViewedContentEntity>>()
        val notSynced = mutableListOf<List<ViewedContentEntity>>()
        backgroundScope.launch(StandardTestDispatcher(testScheduler)) {
            viewedContentDao.getRecentlyViewedContent(
                listOf(
                    ViewedContent.Type.Album.dbValue,
                    ViewedContent.Type.Track.dbValue
                ), 2
            ).toList(mostRecent)
        }
        backgroundScope.launch(StandardTestDispatcher(testScheduler)) {
            viewedContentDao.getNotSynced().toList(notSynced)
        }
        mostRecent.awaitSize(1)
        assertThat(mostRecent.last()).isEmpty()

        notSynced.awaitSize(1)
        assertThat(notSynced.last()).isEmpty()

        // Out of 4 album records, only the max(created) one is returned
        val now = 1000L
        val album1Id = "the album 1"
        val albumRecord1 = viewAlbum(album1Id, now - 300)
        val albumRecord2 = viewAlbum(album1Id, now)
        val albumRecord3 = viewAlbum(album1Id, now - 100)
        val albumRecord4 = viewAlbum(album1Id, now - 200)

        viewedContentDao.insert(albumRecord1)
        notSynced.awaitSize(2)
        viewedContentDao.insert(albumRecord2)
        notSynced.awaitSize(3)
        viewedContentDao.insert(albumRecord3)
        notSynced.awaitSize(4)
        viewedContentDao.insert(albumRecord4)

        notSynced.awaitSize(5)
        assertThat(notSynced.last()).hasSize(4)
        mostRecent.awaitSize(5)
        assertThat(mostRecent.last()).isEqualTo(listOf(albumRecord2.copy(id = 2L)))

        // Since we're filtering only Album and Track, an Artist type record is ignored
        viewedContentDao.insert(viewedArtist("the artist 1", now - 500))

        notSynced.awaitSize(6)
        assertThat(notSynced.last()).hasSize(4 + 1)
        mostRecent.awaitSize(6)
        assertThat(mostRecent.last()).isEqualTo(listOf(albumRecord2.copy(id = 2L)))

        // Similarly to the album, we only see the track with the maximum created timestamp
        val trackId = "track1"
        val trackRecord1 = viewedTrack(trackId, 2000)
        val trackRecord2 = viewedTrack(trackId, 1000)

        viewedContentDao.insert(trackRecord1)
        notSynced.awaitSize(7)
        viewedContentDao.insert(trackRecord2)

        mostRecent.awaitSize(8)
        assertThat(mostRecent.last()).isEqualTo(
            listOf(
                trackRecord1.copy(id = 6L),
                albumRecord2.copy(id = 2L),
            )
        )

        // Another track with a created < than the album won't be shown
        val trackId2 = "track2"
        viewedContentDao.insert(viewedTrack(trackId2, 100))

        mostRecent.awaitSize(9)
        assertThat(mostRecent.last()).isEqualTo(
            listOf(
                trackRecord1.copy(id = 6L),
                albumRecord2.copy(id = 2L),
            )
        )

        // If the created is higher though, it will replace the album
        val trackRecord3 = viewedTrack(trackId2, 3000)
        viewedContentDao.insert(trackRecord3)

        notSynced.awaitSize(10)
        assertThat(notSynced.last()).hasSize(4 + 1 + 2 + 1 + 1)
        mostRecent.awaitSize(10)
        assertThat(mostRecent.last()).isEqualTo(
            listOf(
                trackRecord3.copy(id = 9L),
                trackRecord1.copy(id = 6L),
            )
        )
    }

    private fun viewAlbum(contentId: String, timestamp: Long) =
        viewedContent(contentId, timestamp, ViewedContent.Type.Album)

    private fun viewedArtist(contentId: String, timestamp: Long) =
        viewedContent(contentId, timestamp, ViewedContent.Type.Artist)

    private fun viewedTrack(contentId: String, timestamp: Long) =
        viewedContent(contentId, timestamp, ViewedContent.Type.Track)

    private fun viewedContent(contentId: String, timestamp: Long, type: ViewedContent.Type) =
        ViewedContentEntity(
            id = 0L,
            type = type,
            contentId = contentId,
            created = timestamp,
            synced = false,
        )
}
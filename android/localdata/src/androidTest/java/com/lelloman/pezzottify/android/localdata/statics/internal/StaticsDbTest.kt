package com.lelloman.pezzottify.android.localdata.statics.internal

import android.database.sqlite.SQLiteConstraintException
import androidx.room.Room
import androidx.test.platform.app.InstrumentationRegistry
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.statics.StaticItemType
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticItemFetchStateRecord
import com.lelloman.pezzottify.android.localdata.internal.statics.StaticsDb
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Album
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.internal.statics.model.ArtistDiscography
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Track
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runners.model.TestTimedOutException
import java.util.concurrent.TimeUnit
import kotlin.random.Random

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

        // Keep observing values of artist 1
        val artist1Values = mutableListOf<Artist?>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getArtist(artist1Id).toList(artist1Values)
        }

        artist1Values.awaitSize(1)
        assertThat(artist1Values).isEqualTo(listOf(null))

        val artist1InsertResult = staticsDao.insertArtist(artist1)
        assertThat(artist1InsertResult).isEqualTo(1)

        artist1Values.awaitSize(2)
        assertThat(artist1Values).isEqualTo(listOf(null, artist1))

        val newArtist1 = artist1.copy(name = "new" + artist1.name)
        assertThat(staticsDao.insertArtist(newArtist1)).isEqualTo(2)

        artist1Values.awaitSize(3)
        assertThat(artist1Values).isEqualTo(listOf(null, artist1, newArtist1))

        // artist 2
        val artist2 = randomArtist()
        val artist2InsertResult = staticsDao.insertArtist(artist2)
        assertThat(artist2InsertResult).isEqualTo(3)
        assertThat(staticsDao.getArtist(artist2.id).first()).isEqualTo(artist2)
        val values2 = mutableListOf<Artist?>()
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getArtist(artist2.id).toList(values2)
        }
        values2.awaitSize(1)
        assertThat(values2).isEqualTo(listOf(artist2))

        // Since the SQLite triggers are at table level, we get another newArtist1
        // because the table changed when artist2 was inserted. Alright.
        artist1Values.awaitSize(4)
        assertThat(artist1Values).isEqualTo(listOf(null, artist1, newArtist1, newArtist1))

        // Delete artist 1
        assertThat(staticsDao.deleteArtist(artist1Id)).isEqualTo(1)
        artist1Values.awaitSize(5)
        assertThat(artist1Values).isEqualTo(listOf(null, artist1, newArtist1, newArtist1, null))

        // And now artist 2 values should a double artist2 because of the table trigger
        values2.awaitSize(2)
        assertThat(values2).isEqualTo(listOf(artist2, artist2))
    }

    @Test
    fun handlesTrack() = runTest {
        val track1 = randomTrack()
        val track1Id = track1.id

        // Keep observing values of track 1
        val track1Values = mutableListOf<Track?>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getTrack(track1Id).toList(track1Values)
        }

        track1Values.awaitSize(1)
        assertThat(track1Values).isEqualTo(listOf(null))

        val track1InsertResult = staticsDao.insertTrack(track1)
        assertThat(track1InsertResult).isEqualTo(1)

        track1Values.awaitSize(2)
        assertThat(track1Values).isEqualTo(listOf(null, track1))

        val newTrack1 = track1.copy(name = "new" + track1.name)
        assertThat(staticsDao.insertTrack(newTrack1)).isEqualTo(2)

        track1Values.awaitSize(3)
        assertThat(track1Values).isEqualTo(listOf(null, track1, newTrack1))

        // track 2
        val track2 = randomTrack()
        val track2InsertResult = staticsDao.insertTrack(track2)
        assertThat(track2InsertResult).isEqualTo(3)
        assertThat(staticsDao.getTrack(track2.id).first()).isEqualTo(track2)

        // Delete track 1
        assertThat(staticsDao.deleteTrack(track1Id)).isEqualTo(1)
        track1Values.awaitSize(5)
        assertThat(track1Values).isEqualTo(listOf(null, track1, newTrack1, newTrack1, null))
    }

    @Test
    fun handlesAlbum() = runTest {
        val album1 = randomAlbum()
        val album1Id = album1.id

        // Keep observing values of album 1
        val album1Values = mutableListOf<Album?>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getAlbum(album1Id).toList(album1Values)
        }

        album1Values.awaitSize(1)
        assertThat(album1Values).isEqualTo(listOf(null))

        val album1InsertResult = staticsDao.insertAlbum(album1)
        assertThat(album1InsertResult).isEqualTo(1)

        album1Values.awaitSize(2)
        assertThat(album1Values).isEqualTo(listOf(null, album1))

        val newAlbum1 = album1.copy(name = "new" + album1.name)
        assertThat(staticsDao.insertAlbum(newAlbum1)).isEqualTo(2)

        album1Values.awaitSize(3)
        assertThat(album1Values).isEqualTo(listOf(null, album1, newAlbum1))

        // album 2
        val album2 = randomAlbum()
        val album2InsertResult = staticsDao.insertAlbum(album2)
        assertThat(album2InsertResult).isEqualTo(3)
        assertThat(staticsDao.getAlbum(album2.id).first()).isEqualTo(album2)
        val values2 = mutableListOf<Album?>()

        // Delete album 1
        assertThat(staticsDao.deleteAlbum(album1Id)).isEqualTo(1)
        album1Values.awaitSize(5)
        assertThat(album1Values).isEqualTo(listOf(null, album1, newAlbum1, newAlbum1, null))
    }

    @Test
    fun handlesFetchState() = runTest {
        val state = randomStaticItemFetchStateRecord()
        assertThat(staticItemFetchStateDao.get(state.itemId).first()).isEqualTo(null)
        assertThat(staticItemFetchStateDao.insert(state)).isEqualTo(1)
        assertThat(staticItemFetchStateDao.get(state.itemId).first()).isEqualTo(state)

        val newState = state.copy(loading = true)
        assertThat(staticItemFetchStateDao.insert(newState)).isEqualTo(2)
        assertThat(staticItemFetchStateDao.get(state.itemId).first()).isEqualTo(newState)

        val state2 = state.copy(itemId = "b")
        assertThat(staticItemFetchStateDao.insert(state2)).isEqualTo(3)
        assertThat(staticItemFetchStateDao.get(state2.itemId).first()).isEqualTo(state2)

        val all = mutableListOf<List<StaticItemFetchStateRecord>>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticItemFetchStateDao.getAll().toList(all)
        }

        all.awaitSize(1)
        assertThat(all).isEqualTo(listOf(listOf(newState, state2)))
        assertThat(staticItemFetchStateDao.delete(state.itemId)).isEqualTo(1)
    }

    @Test
    fun handlesArtistDiscography() = runTest {
        val artistDiscography = randomArtistDiscography()
        val artistDiscographyArtistId = artistDiscography.artistId

        // Keep observing values of artistDiscography 1
        val artistDiscographyValues = mutableListOf<ArtistDiscography?>()
        val flowCollectorDispatcher = StandardTestDispatcher(testScheduler)
        backgroundScope.launch(flowCollectorDispatcher) {
            staticsDao.getArtistDiscography(artistDiscographyArtistId)
                .toList(artistDiscographyValues)
        }
        artistDiscographyValues.awaitSize(1)
        assertThat(artistDiscographyValues).isEqualTo(listOf(null))

        // The artist id is constraint to an artist in the artist table
        try {
            staticsDao.insertArtistDiscography(artistDiscography)
            throw AssertionError("Should have thrown an exception")
        } catch (e: Exception) {
            assertThat(e).isInstanceOf(SQLiteConstraintException::class.java)
        }

        // So we need to create an artist first
        val artist = randomArtist().copy(id = artistDiscographyArtistId)
        assertThat(staticsDao.insertArtist(artist)).isEqualTo(1)
        assertThat(staticsDao.insertArtistDiscography(artistDiscography)).isEqualTo(1)

        artistDiscographyValues.awaitSize(2)
        assertThat(artistDiscographyValues).isEqualTo(listOf(null, artistDiscography))

        val newArtistDiscography = artistDiscography.copy(
            albumsIds = listOf("a", "b"),
            featuresIds = listOf("c", "d")
        )
        assertThat(staticsDao.insertArtistDiscography(newArtistDiscography)).isEqualTo(2)

        artistDiscographyValues.awaitSize(3)
        assertThat(artistDiscographyValues).isEqualTo(
            listOf(
                null,
                artistDiscography,
                newArtistDiscography
            )
        )

        // Delete artist 1
        assertThat(staticsDao.deleteArtist(artistDiscographyArtistId)).isEqualTo(1)
        artistDiscographyValues.awaitSize(4)
        assertThat(artistDiscographyValues).isEqualTo(
            listOf(
                null,
                artistDiscography,
                newArtistDiscography,
                null
            )
        )
    }

    private fun randomAlbum() = Album(
        id = Random.nextLong().toString(),
        name = Random.nextLong().toString(),
        genre = emptyList(),
        related = emptyList(),
        coverGroup = emptyList(),
        covers = emptyList(),
        artistsIds = emptyList(),
        discs = emptyList(),
    )

    private fun randomTrack() = Track(
        id = Random.nextLong().toString(),
        name = Random.nextLong().toString(),
        albumId = Random.nextLong().toString(),
        artistsIds = emptyList(),
        durationSeconds = 0,
    )

    private fun randomArtist() = Artist(
        id = Random.nextLong().toString(),
        name = Random.nextLong().toString(),
    )

    private fun randomStaticItemFetchStateRecord() = StaticItemFetchStateRecord(
        itemId = Random.nextLong().toString(),
        loading = Random.nextBoolean(),
        errorReason = null,
        itemType = StaticItemType.Artist,
    )

    private fun randomArtistDiscography() = ArtistDiscography(
        artistId = Random.nextLong().toString(),
        albumsIds = emptyList(),
        featuresIds = emptyList(),
        created = Random.nextLong(),
    )
}
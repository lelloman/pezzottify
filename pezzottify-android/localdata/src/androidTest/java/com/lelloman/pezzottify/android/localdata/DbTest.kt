package com.lelloman.pezzottify.android.localdata

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.localdata.model.Album
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.junit.runner.RunWith


@RunWith(AndroidJUnit4::class)
class DbTest {

    @Test
    fun getsMappedAlbums() {
        runBlocking {
            val context = ApplicationProvider.getApplicationContext<Context>()
            val db = Room
                .inMemoryDatabaseBuilder(context, LocalDb::class.java)
                .build()
            val dao = db.staticsDao()
            val individuals = (0..10).map {
                IndividualArtist(
                    id = "artist$it",
                    displayName = "Artist $it",
                )
            }
            dao.insertIndividuals(individuals)

            val bands = listOf(
                BandArtist(
                    id = "band1",
                    displayName = "Band 1",
                    membersIds = listOf("artist1", "artist2")
                )
            )
            dao.insertBands(bands)

            val tracks = listOf(
                AudioTrack(
                    id = "track1",
                    name = "track 1",
                ),
                AudioTrack(
                    id = "track2",
                    name = "track 2",
                ),
            )
            dao.insertAudioTracks(tracks)

            val album = Album(
                id = "album1",
                name = "The album",
                audioTracksIds = listOf("track1", "track2"),
                artistsIds = listOf("band1", "artist4"),
            )
            dao.insertAlbums(listOf(album))

            val mappedAlbum = dao.getMappedAlbum("album1")
            assertThat(mappedAlbum).isNotNull()
            assertThat(mappedAlbum!!.album.id).isEqualTo(album.id)
            assertThat(mappedAlbum.album.audioTracksIds).hasSize(album.audioTracksIds.size)
            assertThat(mappedAlbum.tracks).isEqualTo(tracks)
        }
    }
}
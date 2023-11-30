package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.controller.model.CreateAlbumRequest
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.ArtistRelation
import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.service.FileStorageService
import com.lelloman.pezzottify.server.utils.*
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.test.annotation.DirtiesContext
import org.springframework.test.context.ActiveProfiles

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class AlbumRestTest {

    @LocalServerPort
    private val port = 0

    @Autowired
    private lateinit var imagesRepository: ImagesRepository

    @Autowired
    private lateinit var audioTrackRepository: AudioTrackRepository

    @Autowired
    private lateinit var fileStorageService: FileStorageService

    @Autowired
    private lateinit var albumsRepo: AlbumRepository

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    private var artist1 = IndividualArtist(displayName = "artist 1")
    private var artist2 = IndividualArtist(displayName = "artist 2")

    private fun createArtist1() {
        artist1 = httpClient.createArtist(artist1).execute().parsedBody()
    }

    private fun createArtist2() {
        artist2 = httpClient.createArtist(artist2).execute().parsedBody()
    }

    @Test
    fun `reads empty albums list`() {
        httpClient.performAdminLogin()

        val albums: Albums = httpClient.get("/api/albums")
            .assertStatus2xx()
            .parsedBody()

        assertThat(albums).isEmpty()
    }

    @Test
    fun `creates an album without images`() {
        httpClient.performAdminLogin()
        createArtist1()
        val album = CreateAlbumRequest(
            name = "The album",
            artistsIds = listOf(artist1.id),
            audioTracksDefs = listOf("Track 1", "Track 2").map {
                CreateAlbumRequest.AudioTrackDef(name = it)
            }
        )
        assertThat(audioTrackRepository.count()).isEqualTo(0)

        val contents = listOf(
            AudioSample.MP3,
            AudioSample.MP3,
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", album.audioTracksDefs.map { it.name }, contents)
            .execute()
            .assertStatus2xx()
            .parsedBody()

        with(createdAlbum) {
            assertThat(id).isNotBlank()
            assertThat(coverImage).isNull()
            assertThat(sideImages).isEmpty()
            assertThat(name).isEqualTo(album.name)
            with(audioTracks) {
                assertThat(this).hasSize(2)
            }
        }
        val tracks = audioTrackRepository.findAll()
        assertThat(tracks).hasSize(2)
        assertThat(tracks).allMatch { !it.orphan }
        assertThat(fileStorageService.totalSize).isEqualTo(3026L)
    }

    @Test
    fun `deletes previously created audio tracks on failure`() {
        httpClient.performAdminLogin()
        val album = CreateAlbumRequest(
            name = "The album",
            audioTracksDefs = listOf("1", "2", "3").map { CreateAlbumRequest.AudioTrackDef(name = it) },
            artistsIds = listOf("1")
        )

        val trackNames = listOf("Track 1", "Track 2", "Invalid track")
        val contents = listOf(
            AudioSample.MP3,
            AudioSample.MP3,
            ByteArray(10000),
        )

        assertThat(audioTrackRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)

        httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", trackNames, contents)
            .addJsonField("audioTracksNames", trackNames)
            .execute()
            .assertStatus(400)

        assertThat(audioTrackRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)
    }

    @Test
    fun `creates an album with images`() {
        httpClient.performAdminLogin()
        createArtist1()
        val album = CreateAlbumRequest(
            name = "The album",
            audioTracksDefs = listOf("1", "2").map { CreateAlbumRequest.AudioTrackDef(name = it) },
            artistsIds = listOf(artist1.id)
        )
        assertThat(audioTrackRepository.count()).isEqualTo(0)

        val contents = listOf(
            AudioSample.MP3,
            AudioSample.MP3,
        )
        val coverBytes = mockPng(100, 100)
        val sideImagesBytes = listOf(
            mockPng(123, 20),
            mockPng(20, 123),
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", album.audioTracksDefs.map { it.name }, contents)
            .addFile("cover", coverBytes)
            .addFiles("sideImages", sideImagesBytes.map { "" }, sideImagesBytes)
            .execute()
            .assertStatus2xx()
            .parsedBody()

        with(createdAlbum) {
            assertThat(id).isNotBlank()
            assertThat(coverImage).isNotNull
            assertThat(coverImage!!.size).isEqualTo(coverBytes.size.toLong())
            assertThat(sideImages).isNotNull
            assertThat(sideImages).hasSize(2)
            assertThat(name).isEqualTo(album.name)
            with(audioTracks) {
                assertThat(this).hasSize(2)
            }
        }
        val tracks = audioTrackRepository.findAll()
        assertThat(tracks).hasSize(2)
        assertThat(tracks).allMatch { !it.orphan }
        assertThat(fileStorageService.totalSize)
            .isEqualTo(AudioSample.MP3.size * 2L + coverBytes.size + sideImagesBytes.sumOf { it.size })

        httpClient.delete("/api/album/${createdAlbum.id}").assertStatus2xx()
        assertThat(albumsRepo.count()).isEqualTo(0)
        assertThat(audioTrackRepository.count()).isEqualTo(0)
        assertThat(imagesRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)
    }

    @Test
    fun `user role can only get ablums`() {
        httpClient.get("/api/albums")
            .assertUnauthorized()
        httpClient.get("/api/album/something")
            .assertUnauthorized()

        httpClient.performUserLogin()

        httpClient.delete("/api/album/something")
            .assertUnauthorized()

        val album = CreateAlbumRequest(
            name = "The album",
            audioTracksDefs = listOf(CreateAlbumRequest.AudioTrackDef(name = "1")),
            artistsIds = listOf("meow")
        )
        httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", listOf(""), listOf(AudioSample.MP3))
            .addFile("cover", mockPng())
            .execute()
            .assertUnauthorized()

        httpClient.get("/api/albums")
            .assertStatus2xx()

        httpClient.get("/api/album/something")
            .assertNotFound()
    }

    @Test
    fun `track gets performer artists from album when not specified`() {
        httpClient.performAdminLogin()

        createArtist1()
        createArtist2()
        val album = CreateAlbumRequest(
            name = "The album",
            audioTracksDefs = listOf(
                CreateAlbumRequest.AudioTrackDef(name = "1"),
                CreateAlbumRequest.AudioTrackDef(name = "2", artists = listOf(ArtistRelation(artistId = artist1.id))),
            ),
            artistsIds = listOf(artist1.id, artist2.id)
        )

        val contents = listOf(
            AudioSample.MP3,
            AudioSample.MP3,
        )
        val coverBytes = mockPng(100, 100)
        val sideImagesBytes = listOf(
            mockPng(123, 20),
            mockPng(20, 123),
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", album.audioTracksDefs.map { it.name }, contents)
            .addFile("cover", coverBytes)
            .addFiles("sideImages", sideImagesBytes.map { "" }, sideImagesBytes)
            .execute()
            .assertStatus2xx()
            .parsedBody()

        with(createdAlbum) {
            assertThat(audioTracks).hasSize(2)
            with(audioTracks[0]) {
                assertThat(name).isEqualTo("1")
                assertThat(artists).hasSize(2)
            }
            with(audioTracks[1]) {
                assertThat(name).isEqualTo("2")
                assertThat(artists).hasSize(1)
            }
        }
    }
}
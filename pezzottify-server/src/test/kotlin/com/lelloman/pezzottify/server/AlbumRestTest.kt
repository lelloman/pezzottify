package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.service.FileStorageService
import com.lelloman.pezzottify.server.utils.Albums
import com.lelloman.pezzottify.server.utils.HttpClient
import com.lelloman.pezzottify.server.utils.MP3_SAMPLE
import com.lelloman.pezzottify.server.utils.mockPng
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
        val album = Album(name = "The album")
        assertThat(audioTrackRepository.count()).isEqualTo(0)

        val trackNames = arrayOf("Track 1", "Track 2")
        val contents = arrayOf(
            MP3_SAMPLE,
            MP3_SAMPLE,
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", trackNames, contents)
            .addJsonField("audioTracksNames", trackNames)
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
        val album = Album(name = "The album")

        val trackNames = arrayOf("Track 1", "Track 2", "Invalid track")
        val contents = arrayOf(
            MP3_SAMPLE,
            MP3_SAMPLE,
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
        val album = Album(name = "The album")
        assertThat(audioTrackRepository.count()).isEqualTo(0)

        val trackNames = arrayOf("Track 1", "Track 2")
        val contents = arrayOf(
            MP3_SAMPLE,
            MP3_SAMPLE,
        )
        val coverBytes = mockPng(100, 100)
        val sideImagesBytes = arrayOf(
            mockPng(123, 20),
            mockPng(20, 123),
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", trackNames, contents)
            .addFile("cover", coverBytes)
            .addJsonField("audioTracksNames", trackNames)
            .addFiles("sideImages", sideImagesBytes.map { "" }.toTypedArray(), sideImagesBytes)
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
            .isEqualTo(MP3_SAMPLE.size * 2L + coverBytes.size + sideImagesBytes.sumOf { it.size })

        httpClient.delete("/api/album/${createdAlbum.id}").assertStatus2xx()
        assertThat(albumsRepo.count()).isEqualTo(0)
        assertThat(audioTrackRepository.count()).isEqualTo(0)
        assertThat(imagesRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)
    }
}
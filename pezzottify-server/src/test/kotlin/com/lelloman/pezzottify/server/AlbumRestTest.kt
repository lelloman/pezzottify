package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.service.FileStorageService
import com.lelloman.pezzottify.server.utils.Albums
import com.lelloman.pezzottify.server.utils.HttpClient
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
    private lateinit var fileStorageService: FileStorageService

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
    fun `creates an album 1`() {
        httpClient.performAdminLogin()
        val album = Album(name = "The album")

        val trackNames = arrayOf("Track 1", "Track 2")
        val contents = arrayOf(
            ByteArray(10) { it.toByte() },
            ByteArray(20) { it.toByte() },
        )
        val createdAlbum: Album = httpClient.multipartPost("/api/album")
            .addJsonField("album", album)
            .addFiles("audioTracks", trackNames, contents)
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
    }
}
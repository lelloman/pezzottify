package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.utils.Artists
import com.lelloman.pezzottify.server.utils.HttpClient
import com.lelloman.pezzottify.server.utils.mockPng
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.test.annotation.DirtiesContext
import org.springframework.test.context.ActiveProfiles

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class ArtistsRestTest {

    @LocalServerPort
    private val port = 0

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    private fun HttpClient.getArtist(id: String): HttpClient.ResponseSpec = this.get("/api/artist/$id")

    private fun HttpClient.createArtist(artist: Artist) = this.multipartPost("/api/artist")
        .addJsonField("artist", artist)

    private fun HttpClient.updateArtist(artist: Artist) = this.multipartPut("/api/artist")
        .addJsonField("artist", artist)

    @Test
    fun `returns 404 for non existent artist id`() {
        httpClient.performAdminLogin()

        httpClient.getArtist("non-existent").assertStatus(404)
    }

    @Test
    fun `cannot create artist without a display name nor a duplicate one`() {
        httpClient.performAdminLogin()

        val artistRequest1 = Artist(displayName = "")
        httpClient.createArtist(artistRequest1)
            .execute()
            .assertStatus(400)

        val artistRequest2 = Artist(displayName = "display")
        httpClient.createArtist(artistRequest2)
            .execute()
            .assertStatus(201)

        httpClient.createArtist(artistRequest2)
            .execute()
            .assertStatus(500) // this should probably be a 400 with a message but whatever
    }

    @Test
    fun `updates artist without image`() {
        httpClient.performAdminLogin()

        val aristRequest1 = Artist(displayName = "display")
        val createdArtist1: Artist = httpClient.createArtist(aristRequest1)
            .execute()
            .assertStatus(201)
            .parsedBody()

        val updatedArtist1: Artist = httpClient.updateArtist(createdArtist1.copy(displayName = "another display"))
            .execute()
            .assertStatus(202)
            .parsedBody()
        assertThat(updatedArtist1).isEqualTo(createdArtist1.copy(displayName = "another display"))

        httpClient.getArtist(updatedArtist1.id)
            .assertStatus2xx()
            .parsedBody<Artist> { assertThat(it).isEqualTo(updatedArtist1) }
    }

    @Test
    fun `creates artist with image`() {
        httpClient.performAdminLogin()

        val artistRequest = Artist(
            firstName = null,
            lastName = "lastName",
            displayName = "The display"
        )
        val imageBytes = mockPng()
        val createdArtist: Artist = httpClient.createArtist(artistRequest)
            .addFile("image", imageBytes)
            .execute()
            .assertStatus(201)
            .parsedBody()
        with(createdArtist) {
            assertThat(image).isNotNull
            assertThat(id).isNotEmpty()
            assertThat(displayName).isEqualTo("The display")
        }

        val artists: Artists = httpClient.get("/api/artists").parsedBody()
        assertThat(artists).hasSize(1)
        val created = artists.firstOrNull { it.displayName == "The display" }
        assertThat(created).isNotNull
        val image = created?.image
        assertThat(image).isNotNull
        assertThat(image!!.width).isEqualTo(10)
        assertThat(image.height).isEqualTo(10)
        assertThat(image.size).isEqualTo(imageBytes.size.toLong())
        assertThat(image.orphan).isFalse()

        val artist: Artist = httpClient.getArtist(createdArtist.id)
            .assertStatus(202)
            .parsedBody()
        assertThat(artist).isEqualTo(createdArtist)
    }

}
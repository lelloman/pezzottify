package com.lelloman.pezzottify.server

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
class ArtistsRestTest {

    @LocalServerPort
    private val port = 0

    @Autowired
    private lateinit var imagesRepository: ImagesRepository

    @Autowired
    private lateinit var fileStorageService: FileStorageService

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    @Test
    fun `returns 404 for non existent artist id`() {
        httpClient.performAdminLogin()

        httpClient.getArtist("non-existent").assertStatus(404)
    }

    @Test
    fun `cannot create artist without a display name nor a duplicate one`() {
        httpClient.performAdminLogin()

        val artistRequest1 = IndividualArtist(displayName = "")
        httpClient.createIndividualArtist(artistRequest1)
            .execute()
            .assertStatus(400)

        val artistRequest2 = IndividualArtist(displayName = "display")
        httpClient.createIndividualArtist(artistRequest2)
            .execute()
            .assertStatus(201)

        httpClient.createIndividualArtist(artistRequest2)
            .execute()
            .assertStatus(500) // this should probably be a 400 with a message but whatever
    }

    @Test
    fun `updates artist without image`() {
        httpClient.performAdminLogin()

        val aristRequest1 = IndividualArtist(displayName = "display")
        val createdArtist1: IndividualArtist = httpClient.createIndividualArtist(aristRequest1)
            .execute()
            .assertStatus(201)
            .parsedBody()

        val updatedArtist1: IndividualArtist =
            httpClient.updateArtist(createdArtist1.copy(displayName = "another display"))
                .execute()
                .assertStatus(202)
                .parsedBody()
        assertThat(updatedArtist1).isEqualTo(createdArtist1.copy(displayName = "another display"))

        httpClient.getArtist(updatedArtist1.id)
            .assertStatus2xx()
            .parsedBody<IndividualArtist> { assertThat(it).isEqualTo(updatedArtist1) }
    }

    @Test
    fun `creates artist with image`() {
        httpClient.performAdminLogin()

        val artistRequest = IndividualArtist(
            firstName = null,
            lastName = "lastName",
            displayName = "The display"
        )
        val imageBytes = mockPng()
        val createdArtist: IndividualArtist = httpClient.createIndividualArtist(artistRequest)
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

        val artist: IndividualArtist = httpClient.getArtist(createdArtist.id)
            .assertStatus(202)
            .parsedBody()
        assertThat(artist).isEqualTo(createdArtist)

        val response = httpClient.get("/api/image/${image.id}")
            .assertStatus(200)
            .rawBody()
        assertThat(response).isNotNull()
        assertThat(response!!.size).isEqualTo(imageBytes.size)
    }

    @Test
    fun `deletes artists image`() {
        httpClient.performAdminLogin()

        val artistRequest = IndividualArtist(
            firstName = null,
            lastName = "lastName",
            displayName = "The display"
        )
        val imageBytes = mockPng()
        val createdArtistId = httpClient.createIndividualArtist(artistRequest)
            .addFile("image", imageBytes)
            .execute()
            .assertStatus(201)
            .parsedBody<IndividualArtist>()
            .id

        assertThat(imagesRepository.count()).isEqualTo(1)
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes.size.toLong())

        val updateArtistRequest = IndividualArtist(
            id = createdArtistId,
            displayName = "a new display",
        )
        val updatedArtist: IndividualArtist = httpClient.updateArtist(updateArtistRequest)
            .execute()
            .assertStatus(202)
            .parsedBody()

        assertThat(updatedArtist.image).isNull()
        assertThat(imagesRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)
    }

    @Test
    fun `replaces artists image`() {
        httpClient.performAdminLogin()

        val artistRequest = IndividualArtist(
            firstName = null,
            lastName = "lastName",
            displayName = "The display"
        )
        val imageBytes1 = mockPng(10, 10)
        val createdArtistId = httpClient.createIndividualArtist(artistRequest)
            .addFile("image", imageBytes1)
            .execute()
            .assertStatus(201)
            .parsedBody<IndividualArtist>()
            .id

        assertThat(imagesRepository.count()).isEqualTo(1)
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes1.size.toLong())

        val imageBytes2 = mockPng(100, 100)
        assertThat(imageBytes2.size).isGreaterThan(imageBytes1.size)
        val updateArtistRequest = IndividualArtist(
            id = createdArtistId,
            displayName = "a new display",
        )
        val updatedArtist: IndividualArtist = httpClient.updateArtist(updateArtistRequest)
            .addFile("image", imageBytes2)
            .execute()
            .assertStatus(202)
            .parsedBody()

        assertThat(updatedArtist.image).isNotNull
        with(updatedArtist.image!!) {
            assertThat(width).isEqualTo(100)
            assertThat(height).isEqualTo(100)
            assertThat(size).isEqualTo(imageBytes2.size.toLong())
        }
        assertThat(imagesRepository.count()).isEqualTo(1)
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes2.size.toLong())
    }
}
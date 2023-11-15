package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.controller.model.CreateBandRequest
import com.lelloman.pezzottify.server.controller.model.UpdateBandRequest
import com.lelloman.pezzottify.server.model.BandArtist
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
        httpClient.createArtist(artistRequest1)
            .execute()
            .assertStatus(400)

        val artistRequest2 = IndividualArtist(displayName = "display")
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

        val aristRequest1 = IndividualArtist(displayName = "display")
        val createdArtist1: IndividualArtist = httpClient.createArtist(aristRequest1)
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
        val createdArtist: IndividualArtist = httpClient.createArtist(artistRequest)
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
        assertThat(created).isInstanceOf(IndividualArtist::class.java)
        val image = (created as IndividualArtist).image
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
        val createdArtistId = httpClient.createArtist(artistRequest)
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
        val createdArtistId = httpClient.createArtist(artistRequest)
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

    @Test
    fun `requires either a band or an individual to create an artist`() {
        httpClient.performAdminLogin()
        val individual = IndividualArtist(displayName = "somebody")
        val band = CreateBandRequest(displayName = "a band", membersIds = listOf())

        httpClient.multipartPost("/api/artist")
            .addJsonField("individual", individual)
            .addJsonField("band", band)
            .execute()
            .assertStatus(400)
            .assertMessage { it.contains("either an individual or a band") }

        httpClient.multipartPost("/api/artist")
            .addFile("image", mockPng())
            .execute()
            .assertStatus(400)
            .assertMessage { it.contains("either an individual or a band") }
    }

    @Test
    fun `must provide members to create a band`() {
        httpClient.performAdminLogin()
        httpClient.createArtist(CreateBandRequest(displayName = "a band", membersIds = emptyList()))
            .execute()
            .assertStatus(400)
            .assertMessage { it.contains("provide at least one member") }
    }

    @Test
    fun `creates a band with one member without image`() {
        httpClient.performAdminLogin()
        val individual = httpClient.createArtist(IndividualArtist(displayName = "An individual"))
            .execute()
            .parsedBody<IndividualArtist>()
        val bandRequest = CreateBandRequest(displayName = "the band", membersIds = listOf(individual.id))

        val createdBand = httpClient.createArtist(bandRequest)
            .execute()
            .assertStatus(201)
            .parsedBody<BandArtist>()

        with(createdBand) {
            assertThat(displayName).isEqualTo(bandRequest.displayName)
            assertThat(members).hasSize(1)
            assertThat(image).isNull()
            assertThat(members[0].displayName).isEqualTo(individual.displayName)
        }
    }

    @Test
    fun `creates a band with two members and an image`() {
        httpClient.performAdminLogin()
        val individual1 = httpClient.createArtist(IndividualArtist(displayName = "An individual"))
            .execute()
            .parsedBody<IndividualArtist>()
        val individual2 = httpClient.createArtist(IndividualArtist(displayName = "Another individual"))
            .execute()
            .parsedBody<IndividualArtist>()

        val bandRequest =
            CreateBandRequest(displayName = "the band", membersIds = listOf(individual1.id, individual2.id))

        val pngBytes = mockPng(100, 120)
        val createdBand = httpClient.createArtist(bandRequest)
            .addFile("image", pngBytes)
            .execute()
            .assertStatus(201)
            .parsedBody<BandArtist>()

        with(createdBand) {
            assertThat(displayName).isEqualTo(bandRequest.displayName)
            assertThat(image?.size).isEqualTo(pngBytes.size.toLong())
            assertThat(members).hasSize(2)
            assertThat(members).anyMatch { it.displayName == individual1.displayName }
            assertThat(members).anyMatch { it.displayName == individual2.displayName }
        }
        assertThat(fileStorageService.totalSize).isEqualTo(pngBytes.size.toLong())
        assertThat(imagesRepository.count()).isEqualTo(1L)
    }

    @Test
    fun `deletes image when band creation fails`() {
        httpClient.performAdminLogin()
        val bandRequest = CreateBandRequest(displayName = "the band", membersIds = listOf())

        val pngBytes = mockPng(100, 120)
        httpClient.createArtist(bandRequest)
            .addFile("image", pngBytes)
            .execute()
            .assertStatus(400)
            .assertMessage { it.contains("at least one member") }

        assertThat(imagesRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0)
    }

    @Test
    fun `fetches both individuals and bands`() {
        httpClient.performAdminLogin()
        val createdIndividuals: List<IndividualArtist> = IntArray(10) { it }
            .map {
                httpClient.createArtist(IndividualArtist(displayName = "Individual $it"))
                    .execute()
                    .assertStatus2xx()
                    .parsedBody<IndividualArtist>()
            }
        assertThat(createdIndividuals).hasSize(10)
        val band1 = httpClient.createArtist(
            CreateBandRequest(
                displayName = "band 1",
                membersIds = listOf(createdIndividuals[0].id, createdIndividuals[1].id)
            )
        ).execute().parsedBody<BandArtist>()

        val band2 = httpClient.createArtist(
            CreateBandRequest(
                displayName = "band 2",
                membersIds = listOf(createdIndividuals[5].id, createdIndividuals[3].id, band1.id)
            )
        ).execute().parsedBody<BandArtist>()

        val artists: Artists = httpClient.get("/api/artists")
            .assertStatus(200)
            .bodyString {
                val a = 1
            }
            .parsedBody()

        assertThat(artists).hasSize(12)
        assertThat(artists.filterIsInstance<BandArtist>()).hasSize(2)
        assertThat(artists.filterIsInstance<IndividualArtist>()).hasSize(10)

        with(artists.filterIsInstance<BandArtist>().first { it.displayName == band1.displayName }) {
            assertThat(members).hasSize(2)
            assertThat(members).anyMatch { it.displayName == "Individual 0" }
            assertThat(members).anyMatch { it.displayName == "Individual 1" }
        }

        with(artists.filterIsInstance<BandArtist>().first { it.displayName == band2.displayName }) {
            assertThat(members).hasSize(3)
            assertThat(members).anyMatch { it.displayName == "Individual 5" }
            assertThat(members).anyMatch { it.displayName == "Individual 3" }
            assertThat(members).anyMatch { it.displayName == "band 1" }
        }
    }

    @Test
    fun `updates band`() {
        httpClient.performAdminLogin()
        val createdIndividuals: List<IndividualArtist> = IntArray(10) { it }
            .map {
                httpClient.createArtist(IndividualArtist(displayName = "Individual $it"))
                    .execute()
                    .assertStatus2xx()
                    .parsedBody<IndividualArtist>()
            }
        assertThat(createdIndividuals).hasSize(10)
        val band = httpClient.createArtist(
            CreateBandRequest(
                displayName = "band 1",
                membersIds = listOf(createdIndividuals[0].id, createdIndividuals[1].id)
            )
        ).execute().parsedBody<BandArtist>()

        with(band) {
            assertThat(id).isEqualTo(band.id)
            assertThat(displayName).isEqualTo("band 1")
            assertThat(members).hasSize(2)
            assertThat(members).anyMatch { it.displayName == "Individual 0" }
            assertThat(members).anyMatch { it.displayName == "Individual 1" }
        }

        val updatedBand = httpClient.updateArtist(
            UpdateBandRequest(
                id = band.id,
                displayName = "A new band 1",
                membersIds = listOf(createdIndividuals[0].id, createdIndividuals[2].id),
                imageId = band.image?.id,
            )
        ).execute().assertStatus2xx().parsedBody<BandArtist>()

        with(updatedBand) {
            assertThat(id).isEqualTo(band.id)
            assertThat(displayName).isEqualTo("A new band 1")
            assertThat(members).hasSize(2)
            assertThat(members).anyMatch { it.displayName == "Individual 0" }
            assertThat(members).anyMatch { it.displayName == "Individual 2" }
        }
    }

    @Test
    fun `deletes band image`() {
        httpClient.performAdminLogin()
        val createdIndividual: IndividualArtist = httpClient
            .createArtist(IndividualArtist(displayName = "Individual"))
            .execute()
            .parsedBody<IndividualArtist>()
        val imageBytes = mockPng()
        val band = httpClient.createArtist(CreateBandRequest("band 1", listOf(createdIndividual.id)))
            .addFile("image", imageBytes)
            .execute().parsedBody<BandArtist>()

        assertThat(band.image).isNotNull
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes.size.toLong())
        assertThat(imagesRepository.count()).isEqualTo(1)

        val updatedBand = httpClient.updateArtist(
            UpdateBandRequest(
                id = band.id,
                displayName = band.displayName,
                membersIds = band.members.map { it.id },
                imageId = null,
            )
        ).execute().assertStatus2xx().parsedBody<BandArtist>()

        assertThat(imagesRepository.count()).isEqualTo(0)
        assertThat(fileStorageService.totalSize).isEqualTo(0L)
        with(updatedBand) {
            assertThat(id).isEqualTo(band.id)
            assertThat(image).isNull()
        }
    }

    @Test
    fun `updates band image`() {
        httpClient.performAdminLogin()
        val createdIndividual: IndividualArtist = httpClient
            .createArtist(IndividualArtist(displayName = "Individual"))
            .execute()
            .parsedBody<IndividualArtist>()
        val imageBytes1 = mockPng()
        val band = httpClient.createArtist(CreateBandRequest("band 1", listOf(createdIndividual.id)))
            .addFile("image", imageBytes1)
            .execute().parsedBody<BandArtist>()

        assertThat(band.image).isNotNull
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes1.size.toLong())
        assertThat(imagesRepository.count()).isEqualTo(1)

        val imageBytes2 = mockPng(200, 300)
        assertThat(imageBytes2.size).isNotEqualTo(imageBytes1.size)
        val updatedBand = httpClient.updateArtist(
            UpdateBandRequest(
                id = band.id,
                displayName = band.displayName,
                membersIds = band.members.map { it.id },
                imageId = null,
            )
        ).addFile("image", imageBytes2).execute().assertStatus2xx().parsedBody<BandArtist>()

        assertThat(imagesRepository.count()).isEqualTo(1)
        assertThat(fileStorageService.totalSize).isEqualTo(imageBytes2.size.toLong())
        with(updatedBand) {
            assertThat(id).isEqualTo(band.id)
            assertThat(image).isNotNull
            assertThat(image!!.id).isNotEqualTo(band.image!!.id)
        }
    }

    @Test
    fun `user role can only get artists`() {
        httpClient.withoutCookies {
            httpClient.get("/api/artists")
                .assertUnauthenticated()
            httpClient.get("/api/artist/something")
                .assertUnauthenticated()
        }

        httpClient.performUserLogin()

        httpClient.multipartPut("/api/artist")
            .addJsonField("asd", Object())
            .execute()
            .assertUnauthorized()

        httpClient.multipartPost("/api/artist")
            .addJsonField("asd", Object())
            .execute()
            .assertUnauthorized()

        httpClient.get("/api/artists")
            .assertStatus2xx()

        httpClient.get("/api/artist/something")
            .assertNotFound()
    }
}
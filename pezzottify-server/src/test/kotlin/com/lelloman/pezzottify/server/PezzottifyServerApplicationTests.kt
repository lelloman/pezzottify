package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.utils.Artists
import com.lelloman.pezzottify.server.utils.HttpClient
import com.lelloman.pezzottify.server.utils.createIndividualArtist
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
class PezzottifyServerApplicationTests {

    @LocalServerPort
    private val port = 0

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }


    @Test
    fun `opens home page`() {
        httpClient.get("/")
            .assertStatus(200)
            .bodyString { body ->
                assertThat(body).isEqualTo("HOME TEST")
            }
    }

    @Test
    fun `creates some artis tracks and albums`() {
        httpClient.performAdminLogin()

        val artistRequests = Array(10) {
            IndividualArtist(
                firstName = if (it % 2 == 0) "First $it" else null,
                lastName = if (it % 2 == 0) "Last $it" else null,
                displayName = "Display $it",
            )
        }
        artistRequests.forEach { request ->
            httpClient.createIndividualArtist(request)
                .addFile("image", mockPng())
                .execute()
                .assertStatus(201)
        }

        val artists: Artists = httpClient.get("/api/artists").parsedBody()
        assertThat(artists).hasSize(artistRequests.size)
    }
}

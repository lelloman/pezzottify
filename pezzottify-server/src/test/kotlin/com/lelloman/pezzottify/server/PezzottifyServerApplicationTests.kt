package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Artist
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.test.context.ActiveProfiles


@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
class PezzottifyServerApplicationTests {

    @LocalServerPort
    private val port = 0

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    class Artists : ArrayList<Artist>()

    private fun performAdminLogin() {
        httpClient.formPost("/login")
            .add("username", "admin")
            .add("password", "admin")
            .execute()
            .assertRedirectTo("/")
    }

    @Test
    fun contextLoads() {
        httpClient.get("/")
            .assertStatus(200)
            .bodyString { body ->
                assertThat(body).isEqualTo("HOME TEST")
            }
    }

    @Test
    fun createsArtistsTracksAndAlbum() {
        performAdminLogin()

        httpClient.get("/api/artists")
            .parsedBody<Artists> { artists ->
                assertThat(artists).hasSize(2)
                val prince = artists.firstOrNull { it.displayName == "Prince" }
                assertThat(prince).isNotNull
            }

        val artistRequest1 = Artist(
            firstName = null,
            lastName = "lastName",
            displayName = "The display"
        )
        httpClient.bodyPost("/api/artist")
            .execute(artistRequest1)
            .assertStatus(200)

        httpClient.get("/api/artists")
            .parsedBody<Artists> { artists ->
                assertThat(artists).hasSize(3)
                val created = artists.firstOrNull { it.displayName == "The display" }
                assertThat(created).isNotNull
            }
    }

}

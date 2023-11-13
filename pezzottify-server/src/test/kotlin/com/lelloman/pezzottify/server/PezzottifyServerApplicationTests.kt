package com.lelloman.pezzottify.server

import com.fasterxml.jackson.core.JsonFactory
import com.fasterxml.jackson.core.JsonGenerator
import com.google.gson.GsonBuilder
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
        httpClient.formPost("/login")
            .add("username", "admin")
            .add("password", "admin")
            .execute()
            .assertRedirectTo("/")

        httpClient.get("/api/artists")
            .parsedBody<Artists> { artists ->
                assertThat(artists).hasSize(2)
                val prince = artists.firstOrNull { it.displayName == "Prince" }
                assertThat(prince).isNotNull
            }
    }

}

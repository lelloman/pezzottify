package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.utils.HttpClient
import org.junit.jupiter.api.Test
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.test.annotation.DirtiesContext
import org.springframework.test.context.ActiveProfiles

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class JwtAuthTest {

    @LocalServerPort
    private val port = 0

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    @Test
    fun `authenticates admin user`() {
        httpClient.get("/me")
            .assertStatus(200)
            .assertMessage { it.contains("You're nobody") }

        httpClient.performAdminLogin()

        httpClient.get("/me")
            .assertStatus(200)
            .assertMessage { it.contains("You're the boss") }

        httpClient.performUserLogin()

        httpClient.get("/me")
            .assertStatus(200)
            .assertMessage { it.contains("You're a regular user") }
    }

}
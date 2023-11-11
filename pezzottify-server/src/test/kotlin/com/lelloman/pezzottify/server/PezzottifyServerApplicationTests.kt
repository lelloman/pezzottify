package com.lelloman.pezzottify.server

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

	@Test
	fun contextLoads() {
		httpClient.get("/")
			.assertStatus(200)
			.bodyString { body ->
				assertThat(body).isEqualTo("HOME TEST")
			}
	}

}

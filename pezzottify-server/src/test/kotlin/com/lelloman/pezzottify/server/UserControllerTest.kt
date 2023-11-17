package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.controller.model.UpdateBookmarkedAlbumsRequest
import com.lelloman.pezzottify.server.controller.model.UserStateResponse
import com.lelloman.pezzottify.server.utils.Artists
import com.lelloman.pezzottify.server.utils.DummyCatalog
import com.lelloman.pezzottify.server.utils.HttpClient
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.test.annotation.DirtiesContext
import org.springframework.test.context.ActiveProfiles

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class UserControllerTest {

    @LocalServerPort
    private val port = 0

    private val baseUrl by lazy { "http://localhost:$port" }
    private val httpClient by lazy { HttpClient(baseUrl) }

    private val dummyCatalog by lazy {
        DummyCatalog.create(httpClient).apply {
            httpClient.get("/api/artists").parsedBody<Artists>()
                .let { assertThat(it.size).isEqualTo(this.bands.size + this.individualArtists.size) }
            httpClient.clearLogin()
        }
    }

    private fun HttpClient.updateUserAlbums(
        albumsToAdd: List<String> = emptyList(),
        albumsToRemove: List<String> = emptyList(),
    ) = jsonBodyPut(
        "/api/user/albums", UpdateBookmarkedAlbumsRequest(
            albumsIdsToAdd = albumsToAdd,
            albumsIdsToRemove = albumsToRemove,
        )
    ).assertStatus(200)

    private fun getUserState(): UserStateResponse = httpClient.get("/api/user/state").parsedBody()

    @Test
    fun `cannot access user state without authentication`() {
        httpClient.clearLogin()
        httpClient.get("/api/user/state").assertUnauthorized()

        httpClient.performAdminLogin()
        httpClient.get("/api/user/state").assertUnauthorized()

        httpClient.performUserLogin()
        with(getUserState()) {
            assertThat(bookmarkedAlbums).isEmpty()
            assertThat(playlists).isEmpty()
        }
    }

    @Test
    fun `adds and removes favorites albums`() {
        assertThat(dummyCatalog.albums).hasSize(2)
        httpClient.performUserLogin()
        assertThat(getUserState().bookmarkedAlbums).isEmpty()

        httpClient.updateUserAlbums(albumsToAdd = listOf(dummyCatalog.albums[0].id))
        with(getUserState()) {
            assertThat(bookmarkedAlbums).hasSize(1)
            assertThat(bookmarkedAlbums.toList()[0]).isEqualTo(dummyCatalog.albums[0].id)
        }

        httpClient.updateUserAlbums(albumsToAdd = listOf(dummyCatalog.albums[1].id))
        with(getUserState()) {
            assertThat(bookmarkedAlbums).hasSize(2)
            assertThat(bookmarkedAlbums).containsAll(listOf(dummyCatalog.albums[0].id, dummyCatalog.albums[1].id))
        }

        httpClient.updateUserAlbums(albumsToRemove = listOf(dummyCatalog.albums[1].id))
        with(getUserState()) {
            assertThat(bookmarkedAlbums).hasSize(1)
            assertThat(bookmarkedAlbums).containsAll(listOf(dummyCatalog.albums[0].id))
        }

        httpClient.updateUserAlbums(
            albumsToAdd = listOf(dummyCatalog.albums[1].id),
            albumsToRemove = listOf(dummyCatalog.albums[0].id),
        )
        with(getUserState()) {
            assertThat(bookmarkedAlbums).hasSize(1)
            assertThat(bookmarkedAlbums).containsAll(listOf(dummyCatalog.albums[1].id))
        }
    }
}
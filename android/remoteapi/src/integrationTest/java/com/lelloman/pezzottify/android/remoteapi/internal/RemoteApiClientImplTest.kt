package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import org.junit.Test

class RemoteApiClientImplTest {

    @Test
    fun `smoke test with actual backend`() = runTest {
        // Setup
        val userHandle = "android-test"
        val password = "asdasd"
        val baseUrl = "http://localhost:3002"
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val hostUrlProvider = object : RemoteApiClient.HostUrlProvider {
            override val hostUrl = MutableStateFlow(baseUrl)
        }
        val client = RemoteApiClientImpl(
            hostUrlProvider = hostUrlProvider,
            okhttpClientBuilder = OkHttpClient.Builder(),
            credentialsProvider = credentialsProvider,
            coroutineScope = this.backgroundScope, // Use background scope so it gets cancelled when test finishes
        )

        // Give retrofit flow time to initialize with the correct URL
        // Background scope coroutines need explicit time advancement
        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Can't get artist without auth token
        val princeId = "R5a2EaR3hamoenG9rDuVn8j"
        val forbiddenResponse = client.getArtist(princeId)
        assertThat(forbiddenResponse).isEqualTo(RemoteApiResponse.Error.Unauthorized)

        // The login endpoint returns the token
        val loginResponse = client.login(userHandle, password)
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Here we can get the artist
        val princeResponse = client.getArtist(princeId)
        assertThat(princeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val prince = (princeResponse as RemoteApiResponse.Success).data
        assertThat(prince.artist.name).isEqualTo("Prince")

        // Let's see the discography
        val princeDiscography = client.getArtistDiscography(princeId)
        assertThat(princeDiscography).isInstanceOf(RemoteApiResponse.Success::class.java)
        val discography = (princeDiscography as RemoteApiResponse.Success).data
        assertThat(discography.albums).isNotEmpty()

        // Let's see an album
        val albumId = discography.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        assertThat(albumResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val album = (albumResponse as RemoteApiResponse.Success).data
        assertThat(album.artists.map { it.id }).contains(princeId)

        // Let's see a track
        val trackId = album.discs.first().tracks.first().id
        val trackResponse = client.getTrack(trackId)
        assertThat(trackResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val track = (trackResponse as RemoteApiResponse.Success).data
        assertThat(track.artists.map { it.artist.id }).contains(princeId)

        // Let's search for prince now
        val searchResponse = client.search("prince")
        assertThat(searchResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val searchResults = (searchResponse as RemoteApiResponse.Success).data
        assertThat(searchResults).isNotEmpty()
        with(searchResults.first()) {
            assertThat(itemId).isEqualTo(princeId)
            assertThat(itemType).isEqualTo(SearchedItemType.Artist)
        }

        // What if we only want only albums?
        val albumSearchResponse = client.search("asd", listOf(RemoteApiClient.SearchFilter.Album))
        assertThat(albumSearchResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val albumSearchResults = (albumSearchResponse as RemoteApiResponse.Success).data
        assertThat(albumSearchResults).isNotEmpty()
        albumSearchResults.forEach {
            assertThat(it.itemType).isEqualTo(SearchedItemType.Album)
        }

        // With an image, we tested all the available static types so far
        val imageId = "ab6761610000e5eb4fcd6f21e60024ae48c3d244"
        val imageResponse = client.getImage(imageId)
        assertThat(imageResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val image = (imageResponse as RemoteApiResponse.Success).data
        assertThat(image.mimeType).isEqualTo("image/jpeg")
    }
}
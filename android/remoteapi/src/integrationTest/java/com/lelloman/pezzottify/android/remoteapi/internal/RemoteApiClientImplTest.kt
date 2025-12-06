package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import org.junit.Test
import java.util.UUID

class RemoteApiClientImplTest {

    private companion object {
        const val BASE_URL = "http://localhost:3002"
        const val USER_HANDLE = "android-test"
        const val PASSWORD = "asdasd"
        const val PRINCE_ID = "R5a2EaR3hamoenG9rDuVn8j"
        const val IMAGE_ID = "ab6761610000e5eb4fcd6f21e60024ae48c3d244"
    }

    private fun createClient(
        credentialsProvider: RemoteApiCredentialsProvider,
        scope: kotlinx.coroutines.CoroutineScope
    ): RemoteApiClientImpl {
        val hostUrlProvider = object : RemoteApiClient.HostUrlProvider {
            override val hostUrl = MutableStateFlow(BASE_URL)
        }
        return RemoteApiClientImpl(
            hostUrlProvider = hostUrlProvider,
            okhttpClientBuilder = OkHttpClient.Builder(),
            credentialsProvider = credentialsProvider,
            coroutineScope = scope,
        )
    }

    private fun createDeviceInfo(suffix: String = "") = DeviceInfo(
        deviceUuid = "android-test-uuid-12345$suffix",
        deviceType = "android",
        deviceName = "Integration Test Device$suffix",
        osInfo = "Android Test",
    )

    @Test
    fun `catalog endpoints - artist, album, track, discography, image, search`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Can't get artist without auth token
        val forbiddenResponse = client.getArtist(PRINCE_ID)
        assertThat(forbiddenResponse).isEqualTo(RemoteApiResponse.Error.Unauthorized)

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo())
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get artist
        val princeResponse = client.getArtist(PRINCE_ID)
        assertThat(princeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val prince = (princeResponse as RemoteApiResponse.Success).data
        assertThat(prince.artist.name).isEqualTo("Prince")

        // Get discography
        val princeDiscography = client.getArtistDiscography(PRINCE_ID)
        assertThat(princeDiscography).isInstanceOf(RemoteApiResponse.Success::class.java)
        val discography = (princeDiscography as RemoteApiResponse.Success).data
        assertThat(discography.albums).isNotEmpty()

        // Get album
        val albumId = discography.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        assertThat(albumResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val album = (albumResponse as RemoteApiResponse.Success).data
        assertThat(album.artists.map { it.id }).contains(PRINCE_ID)

        // Get track
        val trackId = album.discs.first().tracks.first().id
        val trackResponse = client.getTrack(trackId)
        assertThat(trackResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val track = (trackResponse as RemoteApiResponse.Success).data
        assertThat(track.artists.map { it.artist.id }).contains(PRINCE_ID)

        // Search for artist
        val searchResponse = client.search("prince")
        assertThat(searchResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val searchResults = (searchResponse as RemoteApiResponse.Success).data
        assertThat(searchResults).isNotEmpty()
        with(searchResults.first()) {
            assertThat(itemId).isEqualTo(PRINCE_ID)
            assertThat(itemType).isEqualTo(SearchedItemType.Artist)
        }

        // Search with filter (albums only)
        val albumSearchResponse = client.search("asd", listOf(RemoteApiClient.SearchFilter.Album))
        assertThat(albumSearchResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val albumSearchResults = (albumSearchResponse as RemoteApiResponse.Success).data
        assertThat(albumSearchResults).isNotEmpty()
        albumSearchResults.forEach {
            assertThat(it.itemType).isEqualTo(SearchedItemType.Album)
        }

        // Get image
        val imageResponse = client.getImage(IMAGE_ID)
        assertThat(imageResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val image = (imageResponse as RemoteApiResponse.Success).data
        assertThat(image.mimeType).isEqualTo("image/jpeg")
    }

    @Test
    fun `liked content - like, get, unlike`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-liked"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get a track ID for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        assertThat(discographyResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        assertThat(albumResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val trackId = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first().id

        // Initially no liked tracks
        val initialLiked = client.getLikedContent("track")
        assertThat(initialLiked).isInstanceOf(RemoteApiResponse.Success::class.java)
        val initialList = (initialLiked as RemoteApiResponse.Success).data
        assertThat(initialList).doesNotContain(trackId)

        // Like the track
        val likeResponse = client.likeContent("track", trackId)
        assertThat(likeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Now it should be in liked content
        val afterLike = client.getLikedContent("track")
        assertThat(afterLike).isInstanceOf(RemoteApiResponse.Success::class.java)
        val afterLikeList = (afterLike as RemoteApiResponse.Success).data
        assertThat(afterLikeList).contains(trackId)

        // Unlike the track
        val unlikeResponse = client.unlikeContent("track", trackId)
        assertThat(unlikeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Now it should not be in liked content
        val afterUnlike = client.getLikedContent("track")
        assertThat(afterUnlike).isInstanceOf(RemoteApiResponse.Success::class.java)
        val afterUnlikeList = (afterUnlike as RemoteApiResponse.Success).data
        assertThat(afterUnlikeList).doesNotContain(trackId)
    }

    @Test
    fun `sync state - get initial state and events`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-sync"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get sync state
        val syncStateResponse = client.getSyncState()
        assertThat(syncStateResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val syncState = (syncStateResponse as RemoteApiResponse.Success).data
        assertThat(syncState.seq).isAtLeast(0)

        // Get sync events since 0
        val syncEventsResponse = client.getSyncEvents(0)
        assertThat(syncEventsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val syncEvents = (syncEventsResponse as RemoteApiResponse.Success).data
        assertThat(syncEvents.currentSeq).isAtLeast(0)
    }

    @Test
    fun `sync events - action generates event`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-sync-events"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get current sync state
        val initialStateResponse = client.getSyncState()
        assertThat(initialStateResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val initialSeq = (initialStateResponse as RemoteApiResponse.Success).data.seq

        // Get a track ID for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        val trackId = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first().id

        // Like a track (this should generate an event)
        val likeResponse = client.likeContent("track", trackId)
        assertThat(likeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Get events since before the like
        val eventsResponse = client.getSyncEvents(initialSeq)
        assertThat(eventsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val events = (eventsResponse as RemoteApiResponse.Success).data
        assertThat(events.currentSeq).isGreaterThan(initialSeq)
        assertThat(events.events).isNotEmpty()

        // Cleanup - unlike the track
        client.unlikeContent("track", trackId)
    }

    @Test
    fun `listening events - record listening event`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-listening"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get a track ID for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        val track = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first()

        // Create and send a listening event
        val now = System.currentTimeMillis() / 1000
        val listeningEvent = ListeningEventSyncData(
            trackId = track.id,
            sessionId = UUID.randomUUID().toString(),
            startedAt = now - 60,
            endedAt = now,
            durationSeconds = 60,
            trackDurationSeconds = track.durationSecs ?: 180,
            seekCount = 0,
            pauseCount = 1,
            playbackContext = "album:${albumId}",
        )

        val recordResponse = client.recordListeningEvent(listeningEvent)
        assertThat(recordResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
    }

    @Test
    fun `logout - invalidates token`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-logout"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Verify we can access resources
        val artistResponse = client.getArtist(PRINCE_ID)
        assertThat(artistResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Logout
        val logoutResponse = client.logout()
        assertThat(logoutResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // After logout, the token should be invalid (still set in credentialsProvider but server-side invalidated)
        val afterLogoutResponse = client.getArtist(PRINCE_ID)
        assertThat(afterLogoutResponse).isEqualTo(RemoteApiResponse.Error.Unauthorized)
    }
}
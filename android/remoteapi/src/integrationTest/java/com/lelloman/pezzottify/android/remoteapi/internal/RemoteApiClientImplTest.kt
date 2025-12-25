package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import com.lelloman.pezzottify.android.domain.sync.UserSetting
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

        /**
         * Simple OkHttpClientFactory for integration tests.
         */
        private val testOkHttpClientFactory = OkHttpClientFactory()
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
            okHttpClientFactory = testOkHttpClientFactory,
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

    @Test
    fun `popular content - get popular albums and artists`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-popular"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get popular content (may be empty if no listening data, but should succeed)
        val popularResponse = client.getPopularContent()
        assertThat(popularResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val popularContent = (popularResponse as RemoteApiResponse.Success).data
        // Just verify the response structure is valid
        assertThat(popularContent.albums).isNotNull()
        assertThat(popularContent.artists).isNotNull()

        // Test with custom limits
        val limitedResponse = client.getPopularContent(albumsLimit = 5, artistsLimit = 3)
        assertThat(limitedResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
    }

    @Test
    fun `user settings - update and sync generates event`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-settings"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get initial sync state
        val initialStateResponse = client.getSyncState()
        assertThat(initialStateResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val initialSeq = (initialStateResponse as RemoteApiResponse.Success).data.seq

        // Update a setting
        val settingsResponse = client.updateUserSettings(
            listOf(UserSetting.NotifyWhatsNew(value = true))
        )
        assertThat(settingsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Verify it generated a sync event
        val eventsResponse = client.getSyncEvents(initialSeq)
        assertThat(eventsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val events = (eventsResponse as RemoteApiResponse.Success).data
        assertThat(events.currentSeq).isGreaterThan(initialSeq)
        assertThat(events.events).isNotEmpty()

        // Reset the setting
        client.updateUserSettings(listOf(UserSetting.NotifyWhatsNew(value = false)))
    }

    @Test
    fun `liked content - like albums and artists`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-liked-all"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get album ID for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        assertThat(discographyResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id

        // Test liking an album
        val likeAlbumResponse = client.likeContent("album", albumId)
        assertThat(likeAlbumResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        val likedAlbums = client.getLikedContent("album")
        assertThat(likedAlbums).isInstanceOf(RemoteApiResponse.Success::class.java)
        assertThat((likedAlbums as RemoteApiResponse.Success).data).contains(albumId)

        // Cleanup - unlike album
        client.unlikeContent("album", albumId)

        // Test liking an artist
        val likeArtistResponse = client.likeContent("artist", PRINCE_ID)
        assertThat(likeArtistResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        val likedArtists = client.getLikedContent("artist")
        assertThat(likedArtists).isInstanceOf(RemoteApiResponse.Success::class.java)
        assertThat((likedArtists as RemoteApiResponse.Success).data).contains(PRINCE_ID)

        // Cleanup - unlike artist
        client.unlikeContent("artist", PRINCE_ID)
    }

    @Test
    fun `error handling - invalid content IDs return NotFound`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-errors"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Invalid artist ID
        val invalidArtistResponse = client.getArtist("nonexistent-artist-id-12345")
        assertThat(invalidArtistResponse).isEqualTo(RemoteApiResponse.Error.NotFound)

        // Invalid album ID
        val invalidAlbumResponse = client.getAlbum("nonexistent-album-id-12345")
        assertThat(invalidAlbumResponse).isEqualTo(RemoteApiResponse.Error.NotFound)

        // Invalid track ID
        val invalidTrackResponse = client.getTrack("nonexistent-track-id-12345")
        assertThat(invalidTrackResponse).isEqualTo(RemoteApiResponse.Error.NotFound)

        // Invalid image ID
        val invalidImageResponse = client.getImage("nonexistent-image-id-12345")
        assertThat(invalidImageResponse).isEqualTo(RemoteApiResponse.Error.NotFound)

        // Invalid discography ID
        val invalidDiscographyResponse = client.getArtistDiscography("nonexistent-artist-id-12345")
        assertThat(invalidDiscographyResponse).isEqualTo(RemoteApiResponse.Error.NotFound)
    }

    @Test
    fun `login - invalid credentials return error`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Try to login with wrong password
        val wrongPasswordResponse = client.login(USER_HANDLE, "wrongpassword", createDeviceInfo("-wrong-pw"))
        assertThat(wrongPasswordResponse).isInstanceOf(RemoteApiResponse.Error::class.java)
        assertThat(wrongPasswordResponse).isNotInstanceOf(RemoteApiResponse.Success::class.java)

        // Try to login with non-existent user
        val wrongUserResponse = client.login("nonexistent-user", PASSWORD, createDeviceInfo("-wrong-user"))
        assertThat(wrongUserResponse).isInstanceOf(RemoteApiResponse.Error::class.java)
        assertThat(wrongUserResponse).isNotInstanceOf(RemoteApiResponse.Success::class.java)
    }

    @Test
    fun `search - handles various queries gracefully`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-search-misc"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Search with special characters should not crash
        val specialCharsResponse = client.search("test@#\$%")
        assertThat(specialCharsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Search with very long query should not crash
        val longQueryResponse = client.search("a".repeat(100))
        assertThat(longQueryResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Search with numbers
        val numbersResponse = client.search("1999")
        assertThat(numbersResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val numbersResults = (numbersResponse as RemoteApiResponse.Success).data
        // The test catalog has an album and track named "1999"
        assertThat(numbersResults).isNotEmpty()
    }

    @Test
    fun `search - filter by track type`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-search-track"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Search for tracks only (album name "1999" should also match the track)
        val searchResponse = client.search("1999", listOf(RemoteApiClient.SearchFilter.Track))
        assertThat(searchResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val results = (searchResponse as RemoteApiResponse.Success).data
        assertThat(results).isNotEmpty()
        results.forEach {
            assertThat(it.itemType).isEqualTo(SearchedItemType.Track)
        }
    }

    @Test
    fun `multiple listening events - record batch of events`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-listening-batch"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get a track for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        val track = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first()

        // Record multiple listening events
        val now = System.currentTimeMillis() / 1000
        val sessionId = UUID.randomUUID().toString()

        // First partial listen (skipped after 30 seconds)
        val event1 = ListeningEventSyncData(
            trackId = track.id,
            sessionId = sessionId,
            startedAt = now - 300,
            endedAt = now - 270,
            durationSeconds = 30,
            trackDurationSeconds = track.durationSecs ?: 180,
            seekCount = 0,
            pauseCount = 0,
            playbackContext = "album:${albumId}",
        )
        val response1 = client.recordListeningEvent(event1)
        assertThat(response1).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Second full listen
        val event2 = ListeningEventSyncData(
            trackId = track.id,
            sessionId = UUID.randomUUID().toString(),
            startedAt = now - 240,
            endedAt = now - 60,
            durationSeconds = 180,
            trackDurationSeconds = track.durationSecs ?: 180,
            seekCount = 2,
            pauseCount = 1,
            playbackContext = "album:${albumId}",
        )
        val response2 = client.recordListeningEvent(event2)
        assertThat(response2).isInstanceOf(RemoteApiResponse.Success::class.java)
    }

    @Test
    fun `listening history - get listening events`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-listening-history"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // First record a listening event so we have something to retrieve
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        val track = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first()

        val now = System.currentTimeMillis() / 1000
        val event = ListeningEventSyncData(
            trackId = track.id,
            sessionId = UUID.randomUUID().toString(),
            startedAt = now - 300,
            endedAt = now - 60,
            durationSeconds = 240,
            trackDurationSeconds = track.durationSecs ?: 180,
            seekCount = 1,
            pauseCount = 2,
            playbackContext = "album",
        )
        val recordResponse = client.recordListeningEvent(event)
        assertThat(recordResponse).isInstanceOf(RemoteApiResponse.Success::class.java)

        // Now get listening events
        val eventsResponse = client.getListeningEvents(limit = 50, offset = 0)
        assertThat(eventsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val events = (eventsResponse as RemoteApiResponse.Success).data

        // Verify response structure - it's a list directly
        assertThat(events).isNotEmpty()
        val lastEvent = events.first()
        assertThat(lastEvent.trackId).isEqualTo(track.id)
        assertThat(lastEvent.durationSeconds).isEqualTo(240)
        assertThat(lastEvent.seekCount).isEqualTo(1)
        assertThat(lastEvent.pauseCount).isEqualTo(2)
        assertThat(lastEvent.playbackContext).isEqualTo("album")
        assertThat(lastEvent.clientType).isEqualTo("android")
    }

    @Test
    fun `listening history - get listening events with pagination`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-listening-pagination"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get a track for testing
        val discographyResponse = client.getArtistDiscography(PRINCE_ID)
        val albumId = (discographyResponse as RemoteApiResponse.Success).data.albums.first().id
        val albumResponse = client.getAlbum(albumId)
        val track = (albumResponse as RemoteApiResponse.Success).data.discs.first().tracks.first()

        // Record multiple listening events
        val now = System.currentTimeMillis() / 1000
        repeat(5) { i ->
            val event = ListeningEventSyncData(
                trackId = track.id,
                sessionId = UUID.randomUUID().toString(),
                startedAt = now - (i * 60).toLong(),
                endedAt = now - (i * 60 - 30).toLong(),
                durationSeconds = 30,
                trackDurationSeconds = track.durationSecs ?: 180,
                seekCount = 0,
                pauseCount = 0,
                playbackContext = "album",
            )
            client.recordListeningEvent(event)
        }

        // Get first page with limit 2
        val page1Response = client.getListeningEvents(limit = 2, offset = 0)
        assertThat(page1Response).isInstanceOf(RemoteApiResponse.Success::class.java)
        val page1 = (page1Response as RemoteApiResponse.Success).data
        assertThat(page1).hasSize(2)

        // Get second page
        val page2Response = client.getListeningEvents(limit = 2, offset = 2)
        assertThat(page2Response).isInstanceOf(RemoteApiResponse.Success::class.java)
        val page2 = (page2Response as RemoteApiResponse.Success).data
        assertThat(page2).hasSize(2)

        // Events should be different
        val page1Ids = page1.map { it.id }.toSet()
        val page2Ids = page2.map { it.id }.toSet()
        assertThat(page1Ids.intersect(page2Ids)).isEmpty()
    }

    // ==========================================================================
    // Download Manager Tests
    // ==========================================================================

    @Test
    fun `download manager - get limits`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-dm-limits"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get download limits
        val limitsResponse = client.getDownloadLimits()
        assertThat(limitsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val limits = (limitsResponse as RemoteApiResponse.Success).data

        // Verify response structure
        assertThat(limits.maxPerDay).isGreaterThan(0)
        assertThat(limits.maxQueue).isGreaterThan(0)
        assertThat(limits.requestsToday).isAtLeast(0)
        assertThat(limits.inQueue).isAtLeast(0)
    }

    @Test
    fun `download manager - get my requests (initially empty)`() = runTest {
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = createClient(credentialsProvider, this.backgroundScope)

        testScheduler.advanceUntilIdle()
        testScheduler.runCurrent()

        // Login
        val loginResponse = client.login(USER_HANDLE, PASSWORD, createDeviceInfo("-dm-requests"))
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Get my requests - should be empty initially
        val requestsResponse = client.getMyDownloadRequests()
        assertThat(requestsResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val requests = (requestsResponse as RemoteApiResponse.Success).data

        // Verify response structure
        assertThat(requests.requests).isNotNull()
        assertThat(requests.stats).isNotNull()
    }

}
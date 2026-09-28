package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.*
import com.lelloman.pezzottify.android.domain.statics.*
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWork
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import kotlinx.serialization.json.jsonPrimitive
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PlayWorkVersionsTest {
    private val dispatcher = StandardTestDispatcher()
    private val api = mockk<RemoteApiClient>()
    private val player = mockk<PezzottifyPlayer>(relaxed = true)
    private lateinit var controller: RadioCreationController
    private lateinit var play: PlayWorkVersions
    private val work = Work("work", "Composition")

    @Before fun setup() {
        Dispatchers.setMain(dispatcher)
        controller = RadioCreationController()
        play = PlayWorkVersions(GetWork(api), controller, player)
    }
    @After fun teardown() { controller.cancel(); Dispatchers.resetMain() }

    private fun track(id: String, availability: TrackAvailability = TrackAvailability.Available) = TrackResponse(
        track = TrackData(id, id, "album", 1, 1, 1000, availability),
        album = AlbumData("album", "Album", AlbumType.Album), artists = emptyList(),
    )

    @Test fun `all pages including empty pages use cursor and deduplicate with selected version first`() = runTest(dispatcher) {
        coEvery { api.getWork("work", 100, 0, "all") } returns RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("one")), nextOffset = 100, hasMore = true))
        coEvery { api.getWork("work", 100, 100, "all") } returns RemoteApiResponse.Success(WorkPage(work, nextOffset = 200, hasMore = true))
        coEvery { api.getWork("work", 100, 200, "all") } returns RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("two"), track("one"), track("missing", TrackAvailability.Unavailable)), nextOffset = 203, hasMore = false))
        play("work", "two", "Composition")
        advanceUntilIdle()
        verify(exactly = 1) { player.loadRadio(listOf("two", "one"), match {
            it.source == "work_versions" && it.seedEntityType == "track" && it.seedEntityId == "two" && it.settings?.get("work_id")?.jsonPrimitive?.content == "work"
        }, match { it.status == "exhausted" && it.orderedTrackIds == listOf("two", "one") }) }
    }

    @Test fun `unavailable seed is not inserted`() = runTest(dispatcher) {
        coEvery { api.getWork(any(), any(), any(), any()) } returns RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("missing", TrackAvailability.Unavailable), track("one")), nextOffset = 2, hasMore = false))
        play("work", "missing", "Composition")
        advanceUntilIdle()
        verify { player.loadRadio(listOf("one"), any(), any()) }
    }

    @Test fun `broken cursor fails without replacing playback`() = runTest(dispatcher) {
        coEvery { api.getWork(any(), any(), any(), any()) } returns RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("one")), nextOffset = 0, hasMore = true))
        play("work", "one", "Composition")
        advanceUntilIdle()
        assertThat(controller.status.value).isEqualTo(RadioCreationStatus.Error)
        verify { player wasNot Called }
    }

    @Test fun `empty results and network failure preserve playback`() = runTest(dispatcher) {
        coEvery { api.getWork(any(), any(), any(), any()) } returns RemoteApiResponse.Success(WorkPage(work, nextOffset = 0, hasMore = false))
        play("work", "one", "Composition")
        advanceUntilIdle()
        assertThat(controller.status.value).isEqualTo(RadioCreationStatus.Empty)
        coEvery { api.getWork(any(), any(), any(), any()) } returns RemoteApiResponse.Error.Network
        play("work", "one", "Composition")
        advanceUntilIdle()
        assertThat(controller.status.value).isEqualTo(RadioCreationStatus.Error)
        verify { player wasNot Called }
    }

    @Test fun `cancellation and replacement discard unfinished queue`() = runTest(dispatcher) {
        coEvery { api.getWork("work", any(), any(), any()) } coAnswers { delay(1000); RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("old")), nextOffset = 1, hasMore = false)) }
        coEvery { api.getWork("new", any(), any(), any()) } returns RemoteApiResponse.Success(WorkPage(work, tracks = listOf(track("new")), nextOffset = 1, hasMore = false))
        play("work", "old", "Old")
        runCurrent()
        controller.cancel()
        advanceUntilIdle()
        verify { player wasNot Called }
        play("work", "old", "Old")
        runCurrent()
        play("new", "new", "New")
        advanceUntilIdle()
        verify(exactly = 1) { player.loadRadio(listOf("new"), any(), any()) }
        verify(exactly = 0) { player.loadRadio(listOf("old"), any(), any()) }
    }
}

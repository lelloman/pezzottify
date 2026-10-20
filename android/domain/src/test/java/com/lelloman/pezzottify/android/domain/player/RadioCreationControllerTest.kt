package com.lelloman.pezzottify.android.domain.player

import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RadioCreationControllerTest {
    private val dispatcher = StandardTestDispatcher()
    private lateinit var controller: RadioCreationController
    private val context = PlaybackPlaylistContext.Radio(
        source = "basic", seedEntityType = "artist", seedEntityId = "artist-1", seedLabel = "", count = 50,
    )

    @Before fun setup() {
        Dispatchers.setMain(dispatcher)
        controller = RadioCreationController()
    }
    @After fun teardown() {
        controller.cancel()
        Dispatchers.resetMain()
    }

    private fun status() = controller.pending.value?.status
    private fun failed(reason: PendingPlayback.FailureReason) = PendingPlayback.Status.Failed(reason)

    @Test fun `shows creation immediately with context and commits successful result`() = runTest(dispatcher) {
        var committed: PlaybackPlaylistContext.Radio? = null
        controller.start(context) { delay(100); { committed = it } }
        assertEquals(PendingPlayback(PendingPlayback.Status.Creating, context), controller.pending.value)
        advanceUntilIdle()
        assertEquals(context, committed)
        assertNull(controller.pending.value)
    }

    @Test fun `label resolves concurrently with the request and is committed`() = runTest(dispatcher) {
        var committed: PlaybackPlaylistContext.Radio? = null
        controller.start(context, resolveLabel = { delay(10); "The Artist" }) { delay(100); { committed = it } }
        assertEquals("", controller.pending.value?.context?.seedLabel)
        advanceTimeBy(11)
        assertEquals("The Artist", controller.pending.value?.context?.seedLabel)
        assertEquals(PendingPlayback.Status.Creating, status())
        assertNull(committed)
        advanceUntilIdle()
        assertEquals(context.copy(seedLabel = "The Artist"), committed)
        assertNull(controller.pending.value)
    }

    @Test fun `failing label resolution keeps fallback label and still commits`() = runTest(dispatcher) {
        var committed: PlaybackPlaylistContext.Radio? = null
        controller.start(context, resolveLabel = { error("boom") }) { { committed = it } }
        advanceUntilIdle()
        assertEquals(context, committed)
    }

    @Test fun `failure keeps context and resolved label`() = runTest(dispatcher) {
        controller.start(context, resolveLabel = { "The Artist" }) { delay(10); error("network") }
        advanceUntilIdle()
        assertEquals(
            PendingPlayback(failed(PendingPlayback.FailureReason.Error), context.copy(seedLabel = "The Artist")),
            controller.pending.value,
        )
    }

    @Test fun `timeout gives retryable error and cannot commit late`() = runTest(dispatcher) {
        var committed = false
        controller.start(context) { delay(90_000); { committed = true } }
        advanceUntilIdle()
        assertEquals(failed(PendingPlayback.FailureReason.TimedOut), status())
        assertFalse(committed)
        controller.cancel()
        assertNull(controller.pending.value)
    }

    @Test fun `replacement and cancellation discard old requests`() = runTest(dispatcher) {
        val committed = mutableListOf<String>()
        controller.start(context) { delay(100); { committed.add("old") } }
        runCurrent()
        val newContext = context.copy(seedEntityId = "artist-2")
        controller.start(newContext) { { committed.add("new") } }
        assertEquals(newContext, controller.pending.value?.context)
        advanceUntilIdle()
        assertEquals(listOf("new"), committed)
        controller.start(context) { delay(100); { committed.add("cancelled") } }
        controller.cancel()
        advanceUntilIdle()
        assertEquals(listOf("new"), committed)
        assertNull(controller.pending.value)
    }

    @Test fun `superseded label resolution does not leak into new pending playback`() = runTest(dispatcher) {
        controller.start(context, resolveLabel = { delay(50); "Old" }) { delay(100); null }
        runCurrent()
        val newContext = context.copy(seedEntityId = "artist-2", seedLabel = "New")
        controller.start(newContext) { delay(100); null }
        advanceTimeBy(60)
        assertEquals("New", controller.pending.value?.context?.seedLabel)
    }

    @Test fun `failure can be retried and empty result is explicit`() = runTest(dispatcher) {
        var attempts = 0
        var committed = false
        controller.start(context) {
            if (attempts++ == 0) error("network")
            val commit: RadioCommit = { committed = true }
            commit
        }
        advanceUntilIdle()
        assertEquals(failed(PendingPlayback.FailureReason.Error), status())
        controller.retry()
        assertEquals(PendingPlayback.Status.Creating, status())
        advanceUntilIdle()
        assertTrue(committed)
        assertNull(controller.pending.value)
        controller.start(context) { null }
        advanceUntilIdle()
        assertEquals(failed(PendingPlayback.FailureReason.Empty), status())
        assertEquals(context, controller.pending.value?.context)
    }
}

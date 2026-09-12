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
    @Before fun setup() {
        Dispatchers.setMain(dispatcher)
        controller = RadioCreationController()
    }
    @After fun teardown() {
        controller.cancel()
        Dispatchers.resetMain()
    }

    @Test fun `shows creation immediately and commits successful result`() = runTest(dispatcher) {
        var committed = false
        controller.start { delay(100); { committed = true } }
        assertEquals(RadioCreationStatus.Creating, controller.status.value)
        advanceUntilIdle()
        assertTrue(committed)
        assertEquals(RadioCreationStatus.Idle, controller.status.value)
    }

    @Test fun `timeout gives retryable error and cannot commit late`() = runTest(dispatcher) {
        var committed = false
        controller.start { delay(90_000); { committed = true } }
        advanceUntilIdle()
        assertEquals(RadioCreationStatus.TimedOut, controller.status.value)
        assertFalse(committed)
        controller.cancel()
        assertEquals(RadioCreationStatus.Idle, controller.status.value)
    }

    @Test fun `replacement and cancellation discard old requests`() = runTest(dispatcher) {
        val committed = mutableListOf<String>()
        controller.start { delay(100); { committed.add("old"); Unit } }
        runCurrent()
        controller.start { { committed.add("new"); Unit } }
        advanceUntilIdle()
        assertEquals(listOf("new"), committed)
        controller.start { delay(100); { committed.add("cancelled"); Unit } }
        controller.cancel()
        advanceUntilIdle()
        assertEquals(listOf("new"), committed)
    }

    @Test fun `failure can be retried and empty result is explicit`() = runTest(dispatcher) {
        var attempts = 0
        var committed = false
        controller.start {
            if (attempts++ == 0) error("network")
            val commit: () -> Unit = { committed = true }
            commit
        }
        advanceUntilIdle()
        assertEquals(RadioCreationStatus.Error, controller.status.value)
        controller.retry()
        advanceUntilIdle()
        assertTrue(committed)
        controller.start { null }
        advanceUntilIdle()
        assertEquals(RadioCreationStatus.Empty, controller.status.value)
    }
}

package com.lelloman.pezzottify.android.ui.screen.main.assistant

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AssistantConfirmationTest {
    @Test
    fun `confirmation is denied or approved only for the current request`() = runTest {
        val gate = AssistantConfirmation()
        val first = async { gate.confirm("delete_playlist", "playlist A") }
        runCurrent()
        val oldId = gate.request.value!!.id
        gate.respond(oldId, false)
        assertFalse(first.await())
        val second = async { gate.confirm("delete_playlist", "playlist B") }
        runCurrent()
        gate.respond(oldId, true)
        assertFalse(second.isCompleted)
        gate.respond(gate.request.value!!.id, true)
        assertTrue(second.await())
        assertNull(gate.request.value)
    }

    @Test
    fun `cancellation dismisses confirmation and timeout denies it`() = runTest {
        val gate = AssistantConfirmation()
        val pending = async { gate.confirm("delete_playlist", "playlist") }
        runCurrent()
        pending.cancel()
        pending.join()
        assertNull(gate.request.value)
        assertFalse(gate.confirm("delete_playlist", "playlist"))
        assertNull(gate.request.value)
    }
}

package com.lelloman.pezzottify.android.domain.auth

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SessionExpiredEventBusTest {

    @Test
    fun `emit sends event to collectors`() = runTest {
        val eventBus = SessionExpiredEventBus()
        var eventReceived = false

        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            eventBus.events.first()
            eventReceived = true
        }

        eventBus.emit()

        assertThat(eventReceived).isTrue()
        job.cancel()
    }

    @Test
    fun `multiple emits can be collected`() = runTest {
        val eventBus = SessionExpiredEventBus()
        var eventCount = 0

        val job = launch(UnconfinedTestDispatcher(testScheduler)) {
            eventBus.events.collect {
                eventCount++
                if (eventCount >= 2) return@collect
            }
        }

        eventBus.emit()
        eventBus.emit()

        assertThat(eventCount).isEqualTo(2)
        job.cancel()
    }
}

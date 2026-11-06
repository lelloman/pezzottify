package com.lelloman.pezzottify.android.ui.screen.login

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.screen.login.DeviceFlowLoginController.PollResult
import com.lelloman.pezzottify.android.ui.screen.login.DeviceFlowLoginController.StartResult
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.coroutines.coroutineContext

@OptIn(ExperimentalCoroutinesApi::class)
class DeviceFlowLoginControllerTest {

    private val gateway = FakeGateway()

    private fun TestScope.controller() = DeviceFlowLoginController(
        scope = backgroundScope,
        gateway = gateway,
        clock = { testScheduler.currentTime },
    )

    @Test
    fun `start requests a code and shows it with its expiry`() = runTest {
        val controller = controller()

        controller.start()
        runCurrent()

        val state = controller.state.value as DeviceFlowState.Waiting
        assertThat(state.userCode).isEqualTo("CODE-0001")
        assertThat(state.verificationUri).isEqualTo("https://auth.example.com/device")
        assertThat(state.qrContent).isEqualTo("https://auth.example.com/device?user_code=CODE-0001")
        assertThat(state.expiresAtMs).isEqualTo(600_000)
        assertThat(gateway.polls).isEmpty()
    }

    @Test
    fun `pending then approved polls at the server interval and ends approved`() = runTest {
        gateway.pollResults += listOf(PollResult.Pending, PollResult.Approved)
        val controller = controller()

        controller.start()
        advanceTimeBy(4_999)
        assertThat(gateway.polls).isEmpty()

        advanceTimeBy(2)
        assertThat(gateway.polls).containsExactly(5_000L)
        assertThat(controller.state.value).isInstanceOf(DeviceFlowState.Waiting::class.java)

        advanceTimeBy(5_000)
        assertThat(gateway.polls).containsExactly(5_000L, 10_000L).inOrder()
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Approved)

        // Approved is terminal: no more polling.
        advanceTimeBy(60_000)
        assertThat(gateway.polls).hasSize(2)
    }

    @Test
    fun `slow_down raises the interval by five seconds for every following poll`() = runTest {
        gateway.pollResults += listOf(PollResult.SlowDown, PollResult.Pending, PollResult.SlowDown)
        val controller = controller()

        controller.start()
        advanceTimeBy(60_000)

        // 5s, then +10s, +10s, then +15s from there on.
        assertThat(gateway.polls.take(5))
            .containsExactly(5_000L, 15_000L, 25_000L, 40_000L, 55_000L).inOrder()
        assertThat((controller.state.value as DeviceFlowState.Waiting).intervalMs).isEqualTo(15_000)
    }

    @Test
    fun `code expires locally when the deadline passes without approval`() = runTest {
        gateway.expiresInMs = 12_000
        val controller = controller()

        controller.start()
        advanceTimeBy(11_999)
        assertThat(controller.state.value).isInstanceOf(DeviceFlowState.Waiting::class.java)

        advanceTimeBy(2)
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Expired)
        // Polled at 5s and 10s; never after the deadline.
        assertThat(gateway.polls).containsExactly(5_000L, 10_000L).inOrder()

        advanceTimeBy(60_000)
        assertThat(gateway.polls).hasSize(2)
    }

    @Test
    fun `expired_token from the server ends in expired`() = runTest {
        gateway.pollResults += PollResult.Expired
        val controller = controller()

        controller.start()
        advanceTimeBy(5_001)

        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Expired)
    }

    @Test
    fun `access_denied ends in denied and stops polling`() = runTest {
        gateway.pollResults += listOf(PollResult.Pending, PollResult.Denied)
        val controller = controller()

        controller.start()
        advanceTimeBy(30_000)

        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Denied)
        assertThat(gateway.polls).hasSize(2)
    }

    @Test
    fun `restart after expiry requests a new code and polls it`() = runTest {
        gateway.expiresInMs = 6_000
        val controller = controller()
        controller.start()
        advanceTimeBy(6_001)
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Expired)

        // start() alone does not mint a new code after a terminal outcome.
        controller.start()
        runCurrent()
        assertThat(gateway.startCount).isEqualTo(1)

        gateway.expiresInMs = 600_000
        gateway.pollResults += PollResult.Approved
        controller.restart()
        runCurrent()

        assertThat(gateway.startCount).isEqualTo(2)
        val waiting = controller.state.value as DeviceFlowState.Waiting
        assertThat(waiting.userCode).isEqualTo("CODE-0002")
        assertThat(waiting.expiresAtMs).isEqualTo(testScheduler.currentTime + 600_000)

        advanceTimeBy(5_001)
        assertThat(gateway.polledCodes.last()).isEqualTo("device-code-2")
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Approved)
    }

    @Test
    fun `stop halts polling and start resumes the same code`() = runTest {
        val controller = controller()
        controller.start()
        advanceTimeBy(5_001)
        assertThat(gateway.polls).hasSize(1)

        controller.stop()
        advanceTimeBy(60_000)
        assertThat(gateway.polls).hasSize(1)

        controller.start()
        advanceTimeBy(5_001)
        assertThat(gateway.startCount).isEqualTo(1)
        assertThat(gateway.polls).hasSize(2)
        assertThat((controller.state.value as DeviceFlowState.Waiting).userCode).isEqualTo("CODE-0001")
    }

    @Test
    fun `resuming a code that expired while stopped shows expired`() = runTest {
        gateway.expiresInMs = 20_000
        val controller = controller()
        controller.start()
        runCurrent()
        controller.stop()

        advanceTimeBy(30_000)
        controller.start()
        runCurrent()

        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Expired)
        assertThat(gateway.polls).isEmpty()
    }

    @Test
    fun `start while already running does not request a second code`() = runTest {
        val controller = controller()

        controller.start()
        controller.start()
        runCurrent()

        assertThat(gateway.startCount).isEqualTo(1)
    }

    @Test
    fun `transient poll errors are retried and only surface after three in a row`() = runTest {
        gateway.pollResults += listOf(
            PollResult.Error("offline"),
            PollResult.Error("offline"),
            PollResult.Pending,
            PollResult.Error("offline"),
            PollResult.Error("offline"),
            PollResult.Error("still offline"),
        )
        val controller = controller()

        controller.start()
        advanceTimeBy(25_001)
        assertThat(controller.state.value).isInstanceOf(DeviceFlowState.Waiting::class.java)

        advanceTimeBy(5_000)
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Error("still offline"))
        assertThat(gateway.polls).hasSize(6)
    }

    @Test
    fun `start failure shows the error and retry requests a new code`() = runTest {
        gateway.startResults += StartResult.Error("unsupported")
        val controller = controller()

        controller.start()
        runCurrent()
        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Error("unsupported"))

        controller.restart()
        runCurrent()
        assertThat(controller.state.value).isInstanceOf(DeviceFlowState.Waiting::class.java)
    }

    @Test
    fun `invalid host is reported as such`() = runTest {
        gateway.startResults += StartResult.InvalidHost
        val controller = controller()

        controller.start()
        runCurrent()

        assertThat(controller.state.value).isEqualTo(DeviceFlowState.Error(message = null, invalidHost = true))
    }

    private inner class FakeGateway : DeviceFlowLoginController.Gateway {
        var startCount = 0
        var expiresInMs = 600_000L
        val startResults = ArrayDeque<StartResult>()
        val pollResults = ArrayDeque<PollResult>()

        /** Virtual time of each poll. */
        val polls = mutableListOf<Long>()
        val polledCodes = mutableListOf<String>()

        override suspend fun start(): StartResult {
            startCount++
            startResults.removeFirstOrNull()?.let { return it }
            val code = "CODE-%04d".format(startCount)
            return StartResult.Success(
                DeviceFlowLoginController.DeviceCode(
                    deviceCode = "device-code-$startCount",
                    userCode = code,
                    verificationUri = "https://auth.example.com/device",
                    qrContent = "https://auth.example.com/device?user_code=$code",
                    expiresInMs = expiresInMs,
                    intervalMs = 5_000,
                )
            )
        }

        override suspend fun poll(deviceCode: String): PollResult {
            polls += coroutineContext[TestCoroutineScheduler]!!.currentTime
            polledCodes += deviceCode
            return pollResults.removeFirstOrNull() ?: PollResult.Pending
        }
    }
}

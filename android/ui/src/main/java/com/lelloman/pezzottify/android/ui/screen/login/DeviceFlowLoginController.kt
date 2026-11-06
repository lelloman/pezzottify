package com.lelloman.pezzottify.android.ui.screen.login

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Polling state machine for "Sign in from your phone" (OAuth 2.0 device authorization grant).
 *
 * Requests a code through [gateway], then polls at the server's interval until the code is
 * approved, denied or expires. `slow_down` raises the interval by [SLOW_DOWN_INCREMENT_MS];
 * transient errors are retried at the current interval and only surface after
 * [MAX_CONSECUTIVE_ERRORS] in a row. Polling stops on [stop] or when [scope] is cancelled;
 * [start] resumes an unexpired code instead of minting a new one.
 *
 * [clock] is in milliseconds and must match the clock the UI uses to show the remaining time
 * ([DeviceFlowState.Waiting.expiresAtMs]); production uses wall-clock time.
 */
class DeviceFlowLoginController(
    private val scope: CoroutineScope,
    private val gateway: Gateway,
    private val clock: () -> Long,
) {

    interface Gateway {
        suspend fun start(): StartResult

        /** One token-endpoint poll; on [PollResult.Approved] the session has already been stored. */
        suspend fun poll(deviceCode: String): PollResult
    }

    data class DeviceCode(
        val deviceCode: String,
        val userCode: String,
        val verificationUri: String,
        /** What the QR code encodes. */
        val qrContent: String,
        val expiresInMs: Long,
        val intervalMs: Long,
    )

    sealed interface StartResult {
        data class Success(val code: DeviceCode) : StartResult
        data object InvalidHost : StartResult
        data class Error(val message: String?) : StartResult
    }

    sealed interface PollResult {
        data object Pending : PollResult
        data object SlowDown : PollResult
        data object Approved : PollResult
        data object Denied : PollResult
        data object Expired : PollResult
        data class Error(val message: String?) : PollResult
    }

    private val mutableState = MutableStateFlow<DeviceFlowState>(DeviceFlowState.Idle)
    val state: StateFlow<DeviceFlowState> = mutableState.asStateFlow()

    private var job: Job? = null

    /**
     * Starts the flow, or resumes polling a code that is still valid. Does nothing while
     * already running or after a terminal outcome (expired, denied, error, approved): those
     * wait for an explicit [restart].
     */
    fun start() {
        if (job?.isActive == true) return
        when (val current = mutableState.value) {
            DeviceFlowState.Idle, DeviceFlowState.Loading -> job = scope.launch { requestAndPoll() }
            is DeviceFlowState.Waiting -> job = scope.launch { pollUntilDone(current) }
            else -> Unit
        }
    }

    /** Stops polling; the current code is kept so [start] can resume it. */
    fun stop() {
        job?.cancel()
        job = null
    }

    /** Discards the current code (or error) and requests a new one. */
    fun restart() {
        stop()
        mutableState.value = DeviceFlowState.Idle
        start()
    }

    private suspend fun requestAndPoll() {
        mutableState.value = DeviceFlowState.Loading
        when (val result = gateway.start()) {
            is StartResult.Success -> {
                val code = result.code
                val waiting = DeviceFlowState.Waiting(
                    deviceCode = code.deviceCode,
                    userCode = code.userCode,
                    verificationUri = code.verificationUri,
                    qrContent = code.qrContent,
                    expiresAtMs = clock() + code.expiresInMs,
                    intervalMs = code.intervalMs.coerceAtLeast(MIN_INTERVAL_MS),
                )
                mutableState.value = waiting
                pollUntilDone(waiting)
            }

            StartResult.InvalidHost -> mutableState.value = DeviceFlowState.Error(message = null, invalidHost = true)
            is StartResult.Error -> mutableState.value = DeviceFlowState.Error(result.message)
        }
    }

    private suspend fun pollUntilDone(initial: DeviceFlowState.Waiting) {
        var waiting = initial
        var consecutiveErrors = 0
        while (true) {
            val remaining = waiting.expiresAtMs - clock()
            if (remaining <= 0) {
                mutableState.value = DeviceFlowState.Expired
                return
            }
            delay(minOf(waiting.intervalMs, remaining))
            if (clock() >= waiting.expiresAtMs) {
                mutableState.value = DeviceFlowState.Expired
                return
            }

            when (val result = gateway.poll(waiting.deviceCode)) {
                PollResult.Pending -> consecutiveErrors = 0
                PollResult.SlowDown -> {
                    consecutiveErrors = 0
                    waiting = waiting.copy(intervalMs = waiting.intervalMs + SLOW_DOWN_INCREMENT_MS)
                    mutableState.value = waiting
                }

                PollResult.Approved -> {
                    mutableState.value = DeviceFlowState.Approved
                    return
                }

                PollResult.Denied -> {
                    mutableState.value = DeviceFlowState.Denied
                    return
                }

                PollResult.Expired -> {
                    mutableState.value = DeviceFlowState.Expired
                    return
                }

                is PollResult.Error -> {
                    consecutiveErrors++
                    if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
                        mutableState.value = DeviceFlowState.Error(result.message)
                        return
                    }
                }
            }
        }
    }

    companion object {
        const val SLOW_DOWN_INCREMENT_MS = 5_000L
        const val MIN_INTERVAL_MS = 1_000L
        const val MAX_CONSECUTIVE_ERRORS = 3
    }
}

sealed interface DeviceFlowState {
    data object Idle : DeviceFlowState
    data object Loading : DeviceFlowState

    data class Waiting(
        val deviceCode: String,
        val userCode: String,
        val verificationUri: String,
        val qrContent: String,
        val expiresAtMs: Long,
        val intervalMs: Long,
    ) : DeviceFlowState

    data object Expired : DeviceFlowState
    data object Denied : DeviceFlowState
    data class Error(val message: String?, val invalidHost: Boolean = false) : DeviceFlowState

    /** The session is stored; navigation follows from the logged-in state. */
    data object Approved : DeviceFlowState
}

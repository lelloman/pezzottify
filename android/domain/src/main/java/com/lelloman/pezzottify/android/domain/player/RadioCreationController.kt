package com.lelloman.pezzottify.android.domain.player

import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class RadioCreationStatus { Idle, Creating, Error, TimedOut, Empty }

/** Owns creation independently of the screen that requested it. Access on Main. */
@Singleton
class RadioCreationController @Inject constructor() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val mutableStatus = MutableStateFlow(RadioCreationStatus.Idle)
    val status = mutableStatus.asStateFlow()
    private var generation = 0L
    private var job: Job? = null
    private var lastRequest: (suspend () -> (() -> Unit)?)? = null

    fun cancel() {
        generation++
        job?.cancel()
        job = null
        lastRequest = null
        mutableStatus.value = RadioCreationStatus.Idle
    }

    fun retry() { lastRequest?.let(::start) }

    fun start(request: suspend () -> (() -> Unit)?) {
        cancel()
        lastRequest = request
        val token = generation
        mutableStatus.value = RadioCreationStatus.Creating
        job = scope.launch {
            try {
                val commit = withTimeout(60_000) { request() }
                ensureActive()
                if (token != generation) return@launch
                job = null
                if (commit == null) {
                    mutableStatus.value = RadioCreationStatus.Empty
                } else {
                    mutableStatus.value = RadioCreationStatus.Idle
                    lastRequest = null
                    commit()
                }
            } catch (error: TimeoutCancellationException) {
                if (token == generation) mutableStatus.value = RadioCreationStatus.TimedOut
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                if (token == generation) mutableStatus.value = RadioCreationStatus.Error
            }
        }
    }
}

package com.lelloman.pezzottify.android.domain.player

import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Commits a radio queue into the player once it has been created. It receives the final
 * context, including the resolved seed label.
 */
typealias RadioCommit = (PlaybackPlaylistContext.Radio) -> Unit

/**
 * Owns radio creation independently of the screen that requested it and exposes it as a
 * [PendingPlayback]. Superseded or cancelled requests never commit. Access on Main.
 */
@Singleton
class RadioCreationController @Inject constructor() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val mutablePending = MutableStateFlow<PendingPlayback?>(null)
    val pending: StateFlow<PendingPlayback?> = mutablePending.asStateFlow()
    private var generation = 0L
    private var job: Job? = null
    private var lastRequest: Request? = null

    private class Request(
        val context: PlaybackPlaylistContext.Radio,
        val resolveLabel: (suspend () -> String)?,
        val request: suspend () -> RadioCommit?,
    )

    fun cancel() {
        generation++
        job?.cancel()
        job = null
        lastRequest = null
        mutablePending.value = null
    }

    fun retry() {
        val last = lastRequest ?: return
        // Keep any label resolved by the previous attempt.
        val context = mutablePending.value?.context ?: last.context
        start(context, last.resolveLabel.takeIf { context.seedLabel == last.context.seedLabel }, last.request)
    }

    /**
     * Starts creating a radio described by [context]. [resolveLabel], when given, runs
     * concurrently with [request] and replaces the context's seed label once resolved.
     * [request] returns null when there is nothing to play.
     */
    fun start(
        context: PlaybackPlaylistContext.Radio,
        resolveLabel: (suspend () -> String)? = null,
        request: suspend () -> RadioCommit?,
    ) {
        cancel()
        lastRequest = Request(context, resolveLabel, request)
        val token = generation
        mutablePending.value = PendingPlayback(PendingPlayback.Status.Creating, context)
        job = scope.launch {
            try {
                val commit = withTimeout(TIMEOUT_MS) {
                    coroutineScope {
                        val labelJob = resolveLabel?.let { resolve ->
                            launch {
                                val label = try {
                                    resolve()
                                } catch (error: CancellationException) {
                                    throw error
                                } catch (error: Exception) {
                                    null
                                }
                                if (label != null && token == generation) updateLabel(label)
                            }
                        }
                        val result = request()
                        labelJob?.join()
                        result
                    }
                }
                ensureActive()
                if (token != generation) return@launch
                job = null
                if (commit == null) {
                    fail(PendingPlayback.FailureReason.Empty)
                } else {
                    val finalContext = mutablePending.value?.context ?: context
                    mutablePending.value = null
                    lastRequest = null
                    commit(finalContext)
                }
            } catch (error: TimeoutCancellationException) {
                if (token == generation) fail(PendingPlayback.FailureReason.TimedOut)
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                if (token == generation) fail(PendingPlayback.FailureReason.Error)
            }
        }
    }

    private fun updateLabel(label: String) {
        val current = mutablePending.value ?: return
        mutablePending.value = current.copy(context = current.context.copy(seedLabel = label))
    }

    private fun fail(reason: PendingPlayback.FailureReason) {
        val current = mutablePending.value ?: return
        mutablePending.value = current.copy(status = PendingPlayback.Status.Failed(reason))
    }

    private companion object {
        const val TIMEOUT_MS = 60_000L
    }
}

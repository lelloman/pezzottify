package com.lelloman.pezzottify.android.domain.listening

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.app.TimeProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Tracks playback state and generates listening events.
 *
 * Session lifecycle:
 * - Start: When a new track begins playing
 * - Continue: While track plays (accumulating duration, counting pauses/seeks)
 * - Periodic save: Every 10 seconds, update DB record with current progress
 * - End: When track changes, playback stops, or after 5 minutes of inactivity
 */
@OptIn(DelicateCoroutinesApi::class)
@Singleton
class ListeningTracker internal constructor(
    private val player: PezzottifyPlayer,
    private val listeningEventStore: ListeningEventStore,
    private val listeningEventSynchronizer: ListeningEventSynchronizer,
    private val timeProvider: TimeProvider,
    private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    @Inject
    constructor(
        player: PezzottifyPlayer,
        listeningEventStore: ListeningEventStore,
        listeningEventSynchronizer: ListeningEventSynchronizer,
        timeProvider: TimeProvider,
        loggerFactory: LoggerFactory,
    ) : this(
        player,
        listeningEventStore,
        listeningEventSynchronizer,
        timeProvider,
        GlobalScope,
        loggerFactory,
    )

    private val logger = loggerFactory.getLogger(ListeningTracker::class)
    private var currentSession: ActiveSession? = null
    private var lastTrackIndex: Int? = null

    data class ActiveSession(
        val sessionId: String,
        val trackId: String,
        val trackDurationSeconds: Int,
        val startedAt: Long,
        var accumulatedDurationMs: Long = 0,
        var lastResumeTime: Long? = null,
        var lastPauseTime: Long? = null,
        var seekCount: Int = 0,
        var pauseCount: Int = 0,
        val playbackContext: PlaybackPlaylistContext,
        var savedEventId: Long? = null,
    )

    private data class PlaybackState(
        val trackIndex: Int?,
        val isPlaying: Boolean,
    )

    override fun initialize() {
        // Monitor track changes and play state together to avoid race conditions
        scope.launch {
            combine(
                player.currentTrackIndex,
                player.isPlaying,
            ) { trackIndex, isPlaying ->
                PlaybackState(trackIndex, isPlaying)
            }.collect { state ->
                handlePlaybackStateChange(state)
            }
        }

        // Monitor seek events separately
        scope.launch {
            player.seekEvents.collect {
                onSeekEvent()
            }
        }

        // Periodic save loop
        scope.launch {
            while (true) {
                delay(PERIODIC_SAVE_INTERVAL_MS)
                saveCurrentSessionProgress()
            }
        }
    }

    private suspend fun handlePlaybackStateChange(state: PlaybackState) {
        if (state.trackIndex != lastTrackIndex) {
            // Track changed - finalize previous session and start new one
            currentSession?.let { finalizeSession(it) }
            if (state.trackIndex != null) {
                startNewSession(state.trackIndex, state.isPlaying)
            }
            lastTrackIndex = state.trackIndex
        } else {
            // Same track, play state changed
            onPlayStateChanged(state.isPlaying)
        }
    }

    private suspend fun startNewSession(trackIndex: Int, isPlaying: Boolean) {
        val playlist = player.playbackPlaylist.value ?: return
        val trackId = playlist.tracksIds.getOrNull(trackIndex) ?: return

        // Get track duration from player (populated when track loads)
        val trackDurationSeconds = player.currentTrackDurationSeconds.value ?: 0

        currentSession = ActiveSession(
            sessionId = UUID.randomUUID().toString(),
            trackId = trackId,
            trackDurationSeconds = trackDurationSeconds,
            startedAt = timeProvider.nowUtcMs(),
            lastResumeTime = if (isPlaying) timeProvider.nowUtcMs() else null,
            playbackContext = playlist.context,
        )

        logger.debug("Started new listening session for track $trackId")
    }

    private suspend fun onPlayStateChanged(isPlaying: Boolean) {
        val session = currentSession ?: return

        if (isPlaying) {
            // Check for inactivity timeout on resume
            if (shouldStartNewSessionOnResume(session)) {
                finalizeSession(session)
                lastTrackIndex?.let { startNewSession(it, true) }
                return
            }
            session.lastResumeTime = timeProvider.nowUtcMs()
            session.lastPauseTime = null
        } else {
            // Accumulate duration on pause
            session.lastResumeTime?.let { resumeTime ->
                session.accumulatedDurationMs += timeProvider.nowUtcMs() - resumeTime
                session.lastResumeTime = null
                session.lastPauseTime = timeProvider.nowUtcMs()
                session.pauseCount++
            }
        }
    }

    private fun shouldStartNewSessionOnResume(session: ActiveSession): Boolean {
        val pauseTime = session.lastPauseTime ?: return false
        val pauseDuration = timeProvider.nowUtcMs() - pauseTime
        return pauseDuration > INACTIVITY_TIMEOUT_MS
    }

    private fun onSeekEvent() {
        currentSession?.let { it.seekCount++ }
    }

    private suspend fun saveCurrentSessionProgress() {
        val session = currentSession ?: return
        val duration = calculateFinalDuration(session)

        if (duration < MIN_DURATION_THRESHOLD_SEC) return

        val event = createEventFromSession(session, endedAt = null)

        if (session.savedEventId == null) {
            // First save - insert new record
            session.savedEventId = listeningEventStore.saveEvent(event)
            logger.debug("Saved session progress ${session.sessionId}, duration: ${duration}s")
        } else {
            // Update existing record
            listeningEventStore.updateEvent(event.copy(id = session.savedEventId!!))
            logger.debug("Updated session progress ${session.sessionId}, duration: ${duration}s")
        }

        // Wake up the synchronizer to process the new/updated event
        listeningEventSynchronizer.wakeUp()
    }

    private suspend fun finalizeSession(session: ActiveSession) {
        val finalDuration = calculateFinalDuration(session)

        // Only save if minimum threshold met (5 seconds)
        if (finalDuration >= MIN_DURATION_THRESHOLD_SEC) {
            val event = createEventFromSession(session, endedAt = timeProvider.nowUtcMs())

            if (session.savedEventId == null) {
                listeningEventStore.saveEvent(event)
            } else {
                listeningEventStore.updateEvent(event.copy(id = session.savedEventId!!))
            }
            logger.debug("Finalized listening session ${session.sessionId}, duration: ${finalDuration}s")

            // Wake up the synchronizer to process the finalized event
            listeningEventSynchronizer.wakeUp()
        } else {
            // Delete any saved progress if below threshold
            session.savedEventId?.let { listeningEventStore.deleteEvent(it) }
            logger.debug("Session ${session.sessionId} discarded, duration ${finalDuration}s below threshold")
        }

        currentSession = null
    }

    private fun createEventFromSession(session: ActiveSession, endedAt: Long?): ListeningEvent {
        return ListeningEvent(
            trackId = session.trackId,
            sessionId = session.sessionId,
            startedAt = session.startedAt,
            endedAt = endedAt,
            durationSeconds = calculateFinalDuration(session),
            trackDurationSeconds = session.trackDurationSeconds,
            seekCount = session.seekCount,
            pauseCount = session.pauseCount,
            playbackContext = session.playbackContext,
            createdAt = session.startedAt,
        )
    }

    private fun calculateFinalDuration(session: ActiveSession): Int {
        var totalMs = session.accumulatedDurationMs

        // Add current playing segment if still playing
        session.lastResumeTime?.let { resumeTime ->
            totalMs += timeProvider.nowUtcMs() - resumeTime
        }

        return (totalMs / 1000).toInt()
    }

    companion object {
        const val MIN_DURATION_THRESHOLD_SEC = 5
        const val PERIODIC_SAVE_INTERVAL_MS = 10_000L
        const val INACTIVITY_TIMEOUT_MS = 300_000L  // 5 minutes
    }
}

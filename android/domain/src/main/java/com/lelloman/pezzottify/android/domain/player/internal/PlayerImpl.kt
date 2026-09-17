package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackStateStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.CancellationException
import com.lelloman.pezzottify.android.domain.player.RadioContinuation
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.time.Duration.Companion.seconds

class PlayerImpl(
    private val staticsProvider: StaticsProvider,
    loggerFactory: LoggerFactory,
    private val platformPlayer: PlatformPlayer,
    private val configStore: ConfigStore,
    private val userPlaylistStore: UserPlaylistStore,
    private val playbackStateStore: PlaybackStateStore,
    private val userSettingsStore: UserSettingsStore,
    private val remoteApiClient: RemoteApiClient,
    private val coroutineScope: CoroutineScope,
) : PezzottifyPlayer, ControlsAndStatePlayer by platformPlayer {

    private val logger by loggerFactory

    private var loadNexPlaylistJob: Job? = null
    private var statePersistenceJob: Job? = null
    private var smartContinuationJob: Job? = null
    private var smartContinuationRequestJob: Job? = null
    private var smartContinuationInFlightSignature: String? = null
    private var activeRadioSession: String? = null
    private var radioRequestJob: Job? = null
    private var radioGeneration = 0L
    private val mutableRadioContinuationError = MutableStateFlow(false)
    override val radioContinuationError = mutableRadioContinuationError.asStateFlow()
    private var radioErrorSession: String? = null
    private var restorationAttempted = false
    private var restorationInProgress = false

    private val mutablePlaybackPlaylist = MutableStateFlow<PlaybackPlaylist?>(null)
    override val playbackPlaylist = mutablePlaybackPlaylist.asStateFlow()

    override val canGoToPreviousPlaylist: StateFlow<Boolean> = MutableStateFlow(false)

    override val canGoToNextPlaylist: StateFlow<Boolean> = MutableStateFlow(false)

    private fun runOnPlayerThread(block: suspend () -> Unit) =
        coroutineScope.launch(Dispatchers.Main) {
            block()
        }

    override fun initialize() {
        runOnPlayerThread {
            platformPlayer.isActive.collect { isActive ->
                if (!isActive) {
                    mutablePlaybackPlaylist.value = null
                    // Reset restoration flag so we can attempt restoration again
                    // if the service dies while we still have saved state
                    restorationAttempted = false
                }
            }
        }

        // Persist state periodically and when playback state changes
        statePersistenceJob?.cancel()
        statePersistenceJob = coroutineScope.launch(Dispatchers.Main) {
            combine(
                playbackPlaylist,
                platformPlayer.isPlaying,
                platformPlayer.currentTrackIndex,
                platformPlayer.currentTrackProgressSec,
            ) { playlist, isPlaying, trackIndex, progressSec ->
                SaveStateData(playlist, isPlaying, trackIndex, progressSec)
            }.collect { data ->
                val playlist = data.playlist ?: return@collect
                val trackIndex = data.trackIndex ?: return@collect
                val progressSec = data.progressSec ?: 0

                // Save state when we have valid playback info
                playbackStateStore.saveState(
                    playlist = playlist,
                    currentTrackIndex = trackIndex,
                    positionMs = progressSec * 1000L,
                    isPlaying = data.isPlaying,
                )
            }
        }

        smartContinuationJob?.cancel()
        smartContinuationJob = coroutineScope.launch(Dispatchers.Main) {
            combine(
                playbackPlaylist,
                platformPlayer.currentTrackIndex,
                userSettingsStore.isSmartContinuationEnabled,
                platformPlayer.isPlaying,
            ) { playlist, index, enabled, playing ->
                SmartContinuationState(playlist, index, enabled, playing)
            }.collect { state ->
                if (state.playlist?.continuation?.sessionId != activeRadioSession) {
                    cancelRadioRequest()
                    activeRadioSession = state.playlist?.continuation?.sessionId
                }
                maybeFetchSmartContinuation(state)
                maybeContinueRadio(state.playing)
            }
        }
    }

    private data class SaveStateData(
        val playlist: PlaybackPlaylist?,
        val isPlaying: Boolean,
        val trackIndex: Int?,
        val progressSec: Int?,
    )

    private data class SmartContinuationState(
        val playlist: PlaybackPlaylist?,
        val trackIndex: Int?,
        val enabled: Boolean,
        val playing: Boolean,
    )

    private fun maybeFetchSmartContinuation(state: SmartContinuationState) {
        val playlist = state.playlist ?: return
        if (playlist.context is PlaybackPlaylistContext.Radio) return
        val trackIndex = state.trackIndex ?: return
        if (!state.enabled) return
        val remaining = playlist.tracksIds.lastIndex - trackIndex
        if (playlist.tracksIds.isEmpty() || remaining !in 0..1) return

        val signature = "$trackIndex:${playlist.tracksIds.joinToString(",")}"
        if (smartContinuationInFlightSignature == signature) return

        smartContinuationInFlightSignature = signature
        smartContinuationRequestJob?.cancel()
        smartContinuationRequestJob = coroutineScope.launch(Dispatchers.Main) {
            val contextTrackIds = playlist.tracksIds
                .takeLast(10)
            var nextTrackIds = emptyList<String>()
            for (attempt in 0..2) {
                if (attempt > 0) delay(1500L * attempt)
                if (smartContinuationInFlightSignature != signature || !userSettingsStore.isSmartContinuationEnabled.value) break
                val response = withTimeoutOrNull(20_000) {
                    remoteApiClient.getContinuationRecommendations(
                        contextTrackIds = contextTrackIds,
                        excludeTrackIds = playlist.tracksIds,
                        count = 3 - remaining,
                    )
                }
                nextTrackIds = (response as? RemoteApiResponse.Success)?.data.orEmpty()
                if (nextTrackIds.isNotEmpty()) break
            }
            if (smartContinuationInFlightSignature != signature) return@launch
            smartContinuationInFlightSignature = null
            val currentPlaylist = mutablePlaybackPlaylist.value ?: return@launch
            if (currentPlaylist.context is PlaybackPlaylistContext.Radio) return@launch
            val currentIndex = platformPlayer.currentTrackIndex.value ?: return@launch
            val currentSignature = "$currentIndex:${currentPlaylist.tracksIds.joinToString(",")}"
            if (currentSignature != signature) return@launch
            if (!userSettingsStore.isSmartContinuationEnabled.value) return@launch

            val additions = nextTrackIds.distinct().filter { it !in currentPlaylist.tracksIds }
            if (additions.isNotEmpty()) appendTracks(additions, automatic = true)
        }
    }

    private fun cancelRadioRequest() {
        radioGeneration++
        radioRequestJob?.cancel()
        radioRequestJob = null
        mutableRadioContinuationError.value = false
        radioErrorSession = null
    }

    private fun maybeContinueRadio(playing: Boolean) {
        val playlist = mutablePlaybackPlaylist.value ?: return
        val context = playlist.context as? PlaybackPlaylistContext.Radio ?: return
        val index = platformPlayer.currentTrackIndex.value ?: if (playlist.tracksIds.isEmpty()) 0 else return
        val state = playlist.continuation ?: run {
            val restored = RadioContinuation.create(playlist.tracksIds)
            mutablePlaybackPlaylist.value = playlist.copy(continuation =
                if (context.source == "greatest_hits") restored.copy(status = "stopped") else restored)
            return
        }
        if (radioErrorSession != null && radioErrorSession != state.sessionId) {
            mutableRadioContinuationError.value = false
            radioErrorSession = null
        }
        if (!playing || state.status != "active" || (playlist.tracksIds.isNotEmpty() && playlist.tracksIds.lastIndex - index !in 0..1) ||
            radioRequestJob?.isActive == true || radioErrorSession == state.sessionId) return
        val token = ++radioGeneration
        radioRequestJob = coroutineScope.launch(Dispatchers.Main) {
            fun valid() = token == radioGeneration && mutablePlaybackPlaylist.value?.continuation === state
            try {
                var cursor = state.nextIndex
                val trackIds = if (state.strategy == "ranked_snapshot") {
                    state.nextBatch().let { (ids, next) -> cursor = next; ids }
                } else {
                    var ids: List<String>? = null
                    for (attempt in 0..2) {
                        if (attempt > 0) delay(1500L * attempt)
                        if (!valid()) return@launch
                        val response = withTimeoutOrNull(20_000) {
                            remoteApiClient.continueRadio(context, playlist.tracksIds.takeLast(10), state.seenTrackIds)
                        }
                        if (response is RemoteApiResponse.Success) { ids = response.data; break }
                    }
                    ids ?: error("Radio continuation failed")
                }
                if (!valid()) return@launch
                val current = mutablePlaybackPlaylist.value ?: return@launch
                val currentIndex = platformPlayer.currentTrackIndex.value ?: if (current.tracksIds.isEmpty()) 0 else return@launch
                val seen = state.seenTrackIds.toHashSet()
                val additions = trackIds.distinct().filter { it !in seen }
                val trim = minOf(currentIndex, (current.tracksIds.size + additions.size - 500).coerceAtLeast(0))
                // Remove only preceding media items: Media3 preserves the current item and position.
                repeat(trim) { platformPlayer.removeMediaItem(0) }
                val baseUrl = configStore.baseUrl.value
                platformPlayer.addMediaItems(additions.map { "$baseUrl/v1/content/stream/$it" })
                if (current.tracksIds.isEmpty() && additions.isNotEmpty()) platformPlayer.loadTrack(0, 0)
                mutablePlaybackPlaylist.value = current.copy(
                    tracksIds = current.tracksIds.drop(trim) + additions,
                    continuation = state.appended(additions, cursor),
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                if (valid()) {
                    radioErrorSession = state.sessionId
                    mutableRadioContinuationError.value = true
                    logger.warn("Radio continuation failed: ${error.message}")
                }
            }
        }
    }

    override fun retryRadioContinuation() {
        runOnPlayerThread {
            radioErrorSession = null
            mutableRadioContinuationError.value = false
            maybeContinueRadio(true)
        }
    }

    override fun dismissRadioContinuationError() { mutableRadioContinuationError.value = false }

    private fun editedContinuation(playlist: PlaybackPlaylist, trackIds: List<String>): RadioContinuation? {
        if (playlist.context !is PlaybackPlaylistContext.Radio) return playlist.continuation
        cancelRadioRequest()
        return (playlist.continuation ?: RadioContinuation.create(playlist.tracksIds))
            .edited(trackIds, userSettingsStore.keepRadioOnQueueEdit.value)
    }

    /**
     * Attempts to restore a previously saved playback state.
     * Called automatically when the user tries to play but the service is not active.
     *
     * @return true if restoration was attempted, false if no saved state exists
     */
    override suspend fun tryRestoreState(): Boolean {
        if (restorationAttempted) return false

        val savedState = playbackStateStore.loadState() ?: return false
        restorationAttempted = true

        logger.info("Restoring saved playback state: ${savedState.playlist.tracksIds.size} tracks, index=${savedState.currentTrackIndex}")

        // Restore the playlist
        mutablePlaybackPlaylist.value = savedState.playlist

        // Load tracks into player
        val baseUrl = configStore.baseUrl.value
        val urls = savedState.playlist.tracksIds.map { "$baseUrl/v1/content/stream/$it" }
        platformPlayer.loadPlaylist(urls, playWhenReady = savedState.isPlaying)

        // Restore both the selected track and its exact position.
        platformPlayer.loadTrack(savedState.currentTrackIndex, savedState.positionMs)

        // Clear saved state after successful restoration
        playbackStateStore.clearState()

        return true
    }

    /**
     * Override togglePlayPause to attempt state restoration if player is inactive.
     */
    override fun togglePlayPause() {
        if (platformPlayer.isPlaying.value) cancelRadioRequest()
        // If restoration is in progress, ignore this toggle to avoid race conditions
        if (restorationInProgress) {
            logger.debug("togglePlayPause() - restoration in progress, ignoring")
            return
        }

        if (!platformPlayer.isActive.value) {
            // Player is not active - try to restore saved state
            restorationInProgress = true
            runOnPlayerThread {
                val restored = tryRestoreState()
                restorationInProgress = false

                if (!restored) {
                    // No saved state to restore, forward toggle to platform player
                    platformPlayer.togglePlayPause()
                }
                // If restored, playback starts automatically via loadPlaylist
            }
        } else {
            platformPlayer.togglePlayPause()
        }
    }

    /**
     * Override setIsPlaying to attempt state restoration if player is inactive.
     */
    override fun setIsPlaying(isPlaying: Boolean) {
        if (!isPlaying) cancelRadioRequest()
        // If restoration is in progress, ignore this call to avoid race conditions
        if (restorationInProgress) {
            logger.debug("setIsPlaying($isPlaying) - restoration in progress, ignoring")
            return
        }

        if (isPlaying && !platformPlayer.isActive.value) {
            // Trying to play but player is not active - try to restore saved state
            restorationInProgress = true
            runOnPlayerThread {
                val restored = tryRestoreState()
                restorationInProgress = false

                if (!restored) {
                    // No saved state to restore, forward to platform player
                    platformPlayer.setIsPlaying(isPlaying)
                }
                // If restored, playback starts automatically via loadPlaylist
            }
        } else {
            platformPlayer.setIsPlaying(isPlaying)
        }
    }

    override fun loadAlbum(albumId: String, startTrackId: String?) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                val loadedAlbum = withTimeoutOrNull(2.seconds) {
                    staticsProvider.provideAlbum(albumId)
                        .filterIsInstance<StaticsItem.Loaded<Album>>()
                        .first()
                }
                if (loadedAlbum != null) {
                    val tracksIds = loadedAlbum.data.discs.flatMap { it.tracksIds }

                    logger.info("Loading new track list into platform player.")
                    val baseUrl = configStore.baseUrl.value
                    val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                    platformPlayer.loadPlaylist(urls, playWhenReady = true)

                    // Set playlist state after starting playback
                    // This triggers metadata loading which can be slow
                    mutablePlaybackPlaylist.value = PlaybackPlaylist(
                        context = PlaybackPlaylistContext.Album(albumId),
                        tracksIds = tracksIds,
                    )

                    // If a specific track was requested, start from that track
                    if (startTrackId != null) {
                        val startIndex = tracksIds.indexOf(startTrackId)
                        if (startIndex >= 0) {
                            platformPlayer.loadTrackIndex(startIndex)
                            logger.info("Starting album $albumId at track index $startIndex (trackId: $startTrackId)")
                        } else {
                            logger.warn("Start track $startTrackId not found in album $albumId, starting from beginning")
                        }
                    }

                    logger.info("Loaded album $albumId")
                }
            }
        }
    }

    override fun addAlbumToPlaylist(albumId: String) {
        runOnPlayerThread {
            val loadedAlbum = withTimeoutOrNull(2.seconds) {
                staticsProvider.provideAlbum(albumId)
                    .filterIsInstance<StaticsItem.Loaded<Album>>()
                    .first()
            }
            if (loadedAlbum != null) {
                val tracksIds = loadedAlbum.data.discs.flatMap { it.tracksIds }
                addTracksToPlaylist(tracksIds)
                logger.info("Added album $albumId (${tracksIds.size} tracks) to playlist")
            }
        }
    }

    override fun loadUserPlaylist(userPlaylistId: String, startTrackId: String?) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                val playlist = withTimeoutOrNull(2.seconds) {
                    userPlaylistStore.getPlaylist(userPlaylistId).first()
                }
                if (playlist != null && playlist.trackIds.isNotEmpty()) {
                    val tracksIds = playlist.trackIds

                    logger.info("Loading user playlist into platform player.")
                    val baseUrl = configStore.baseUrl.value
                    val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                    platformPlayer.loadPlaylist(urls, playWhenReady = true)

                    // Set playlist state after starting playback
                    mutablePlaybackPlaylist.value = PlaybackPlaylist(
                        context = PlaybackPlaylistContext.UserPlaylist(userPlaylistId, isEdited = false),
                        tracksIds = tracksIds,
                    )

                    // If a specific track was requested, start from that track
                    if (startTrackId != null) {
                        val startIndex = tracksIds.indexOf(startTrackId)
                        if (startIndex >= 0) {
                            platformPlayer.loadTrackIndex(startIndex)
                            logger.info("Starting user playlist $userPlaylistId at track index $startIndex (trackId: $startTrackId)")
                        } else {
                            logger.warn("Start track $startTrackId not found in user playlist $userPlaylistId, starting from beginning")
                        }
                    }

                    logger.info("Loaded user playlist $userPlaylistId with ${tracksIds.size} tracks")
                } else {
                    logger.warn("User playlist $userPlaylistId not found or empty")
                }
            }
        }
    }

    override fun addUserPlaylistToQueue(userPlaylistId: String) {
        runOnPlayerThread {
            val playlist = withTimeoutOrNull(2.seconds) {
                userPlaylistStore.getPlaylist(userPlaylistId).first()
            }
            if (playlist != null && playlist.trackIds.isNotEmpty()) {
                addTracksToPlaylist(playlist.trackIds)
                logger.info("Added user playlist $userPlaylistId (${playlist.trackIds.size} tracks) to queue")
            } else {
                logger.warn("User playlist $userPlaylistId not found or empty")
            }
        }
    }

    override fun loadSingleTrack(trackId: String) {
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            loadNexPlaylistJob = runOnPlayerThread {
                val baseUrl = configStore.baseUrl.value
                val url = "$baseUrl/v1/content/stream/$trackId"
                platformPlayer.loadPlaylist(listOf(url), playWhenReady = true)

                // Set playlist state after starting playback
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = PlaybackPlaylistContext.UserMix,
                    tracksIds = listOf(trackId),
                )
                logger.info("Loaded single track $trackId")
            }
        }
    }

    override fun loadTrackIds(trackIds: List<String>) {
        if (trackIds.isEmpty()) return
        runOnPlayerThread {
            loadNexPlaylistJob?.cancel()
            val baseUrl = configStore.baseUrl.value
            val urls = trackIds.map { "$baseUrl/v1/content/stream/$it" }
            platformPlayer.loadPlaylist(urls, playWhenReady = true)
            mutablePlaybackPlaylist.value = PlaybackPlaylist(
                context = PlaybackPlaylistContext.UserMix,
                tracksIds = trackIds,
            )
            logger.info("Loaded radio/user mix with ${trackIds.size} tracks")
        }
    }

    override fun loadRadio(trackIds: List<String>, context: PlaybackPlaylistContext.Radio, continuation: RadioContinuation?) {
        if (trackIds.isEmpty()) return
        runOnPlayerThread {
            cancelRadioRequest()
            loadNexPlaylistJob?.cancel()
            if (context.source == "greatest_hits") {
                if (platformPlayer.shuffleEnabled.value) platformPlayer.toggleShuffle()
                val repeatSteps = when (platformPlayer.repeatMode.value) {
                    com.lelloman.pezzottify.android.domain.player.RepeatMode.OFF -> 0
                    com.lelloman.pezzottify.android.domain.player.RepeatMode.ALL -> 2
                    com.lelloman.pezzottify.android.domain.player.RepeatMode.ONE -> 1
                }
                repeat(repeatSteps) { platformPlayer.cycleRepeatMode() }
            }
            val baseUrl = configStore.baseUrl.value
            val urls = trackIds.map { "$baseUrl/v1/content/stream/$it" }
            platformPlayer.loadPlaylist(urls, playWhenReady = true)
            mutablePlaybackPlaylist.value = PlaybackPlaylist(
                continuation = continuation ?: RadioContinuation.create(trackIds),
                context = context,
                tracksIds = trackIds,
            )
            logger.info("Loaded radio ${context.source}:${context.seedEntityType}:${context.seedEntityId} with ${trackIds.size} tracks")
        }
    }

    override fun forward10Sec() {
        platformPlayer.forward10Sec( )
    }

    override fun rewind10Sec() {
        platformPlayer.rewind10Sec()
    }

    override fun stop() {
        cancelRadioRequest()
        platformPlayer.stop()
    }

    override fun setVolume(volume: Float) {
        platformPlayer.setVolume(volume)
    }

    override fun setMuted(isMuted: Boolean) {
        platformPlayer.setMuted(isMuted)
    }

    override fun loadTrackIndex(index: Int) {
        platformPlayer.loadTrackIndex(index)
    }

    override fun goToPreviousPlaylist() {
        logger.warn("goToPreviousPlaylist() is not yet implemented")
    }

    override fun goToNextPlaylist() {
        logger.warn("goToNextPlaylist() is not yet implemented")
    }

    override fun moveTrack(fromIndex: Int, toIndex: Int) {
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value ?: return@runOnPlayerThread
            if (fromIndex !in currentPlaylist.tracksIds.indices ||
                toIndex !in currentPlaylist.tracksIds.indices ||
                fromIndex == toIndex
            ) {
                return@runOnPlayerThread
            }

            val reorderedTracks = currentPlaylist.tracksIds.toMutableList().apply {
                add(toIndex, removeAt(fromIndex))
            }
            val newContext = when (val ctx = currentPlaylist.context) {
                is PlaybackPlaylistContext.Album -> PlaybackPlaylistContext.UserMix
                is PlaybackPlaylistContext.UserPlaylist -> ctx.copy(isEdited = true)
                is PlaybackPlaylistContext.UserMix -> ctx
                is PlaybackPlaylistContext.Radio -> ctx.copy(isEdited = true)
            }

            platformPlayer.moveMediaItem(fromIndex, toIndex)
            mutablePlaybackPlaylist.value = PlaybackPlaylist(newContext, reorderedTracks, editedContinuation(currentPlaylist, reorderedTracks))
            logger.info("Moved track from index $fromIndex to $toIndex")
        }
    }

    override fun addTracksToPlaylist(tracksIds: List<String>) {
        appendTracks(tracksIds, automatic = false)
    }

    private fun appendTracks(tracksIds: List<String>, automatic: Boolean) {
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value
            if (currentPlaylist != null) {
                // Add tracks to the existing playlist
                val newTracksIds = currentPlaylist.tracksIds + tracksIds
                val newContext = if (automatic) currentPlaylist.context else when (val ctx = currentPlaylist.context) {
                    is PlaybackPlaylistContext.Album -> PlaybackPlaylistContext.UserMix
                    is PlaybackPlaylistContext.UserPlaylist -> ctx.copy(isEdited = true)
                    is PlaybackPlaylistContext.UserMix -> ctx
                    is PlaybackPlaylistContext.Radio -> ctx.copy(isEdited = true)
                }
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = newContext,
                    tracksIds = newTracksIds,
                    continuation = if (automatic) currentPlaylist.continuation else editedContinuation(currentPlaylist, newTracksIds),
                )
                // Add new tracks to platform player
                val baseUrl = configStore.baseUrl.value
                val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                platformPlayer.addMediaItems(urls)
                logger.info("Added ${tracksIds.size} tracks to playlist")
            } else {
                // No playlist exists, create a new UserMix playlist
                mutablePlaybackPlaylist.value = PlaybackPlaylist(
                    context = PlaybackPlaylistContext.UserMix,
                    tracksIds = tracksIds,
                )
                val baseUrl = configStore.baseUrl.value
                val urls = tracksIds.map { "$baseUrl/v1/content/stream/$it" }
                platformPlayer.loadPlaylist(urls, playWhenReady = true)
                logger.info("Created new UserMix playlist with ${tracksIds.size} tracks")
            }
        }
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value ?: return@runOnPlayerThread
            val trackIndex = currentPlaylist.tracksIds.indexOf(trackId)
            if (trackIndex < 0) {
                logger.warn("Track $trackId not found in playlist, cannot remove")
                return@runOnPlayerThread
            }

            removeTrackAtIndexInternal(trackIndex)
        }
    }

    override fun removeTrackAtIndex(index: Int) {
        runOnPlayerThread {
            val currentPlaylist = mutablePlaybackPlaylist.value ?: return@runOnPlayerThread
            if (index !in currentPlaylist.tracksIds.indices) {
                logger.warn("Track index $index out of bounds, cannot remove")
                return@runOnPlayerThread
            }
            removeTrackAtIndexInternal(index)
        }
    }

    private fun removeTrackAtIndexInternal(trackIndex: Int) {
        val currentPlaylist = mutablePlaybackPlaylist.value ?: return
        val newTracksIds = currentPlaylist.tracksIds.toMutableList().apply {
            removeAt(trackIndex)
        }

        // Update context to reflect the playlist has been modified
        val newContext = when (val ctx = currentPlaylist.context) {
            is PlaybackPlaylistContext.Album -> PlaybackPlaylistContext.UserMix
            is PlaybackPlaylistContext.UserPlaylist -> ctx.copy(isEdited = true)
            is PlaybackPlaylistContext.UserMix -> ctx
            is PlaybackPlaylistContext.Radio -> ctx.copy(isEdited = true)
        }

        mutablePlaybackPlaylist.value = PlaybackPlaylist(
            context = newContext,
            tracksIds = newTracksIds,
            continuation = editedContinuation(currentPlaylist, newTracksIds),
        )

        // Remove from platform player
        platformPlayer.removeMediaItem(trackIndex)
        if (newTracksIds.isEmpty()) maybeContinueRadio(true)
        logger.info("Removed track at index $trackIndex from playlist")
    }

    override fun clearSession() {
        cancelRadioRequest()
        loadNexPlaylistJob?.cancel()
        loadNexPlaylistJob = null
        smartContinuationRequestJob?.cancel()
        smartContinuationRequestJob = null
        smartContinuationInFlightSignature = null
        mutablePlaybackPlaylist.value = null
        platformPlayer.clearSession()
        // Clear persisted state on logout so we don't restore stale session
        coroutineScope.launch {
            playbackStateStore.clearState()
        }
        restorationAttempted = false
        restorationInProgress = false
        logger.info("Cleared player session")
    }

    override fun toggleShuffle() {
        platformPlayer.toggleShuffle()
    }

    override fun cycleRepeatMode() {
        platformPlayer.cycleRepeatMode()
    }

    override fun retry() {
        platformPlayer.retry()
    }
}

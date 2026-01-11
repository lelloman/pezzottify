package com.lelloman.pezzottify.android.localplayer

import android.app.Application
import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.localplayer.data.LocalPlaylistDao
import com.lelloman.pezzottify.android.localplayer.data.LocalPlaylistEntity
import com.lelloman.pezzottify.android.localplayer.data.LocalPlaylistTrackEntity
import com.lelloman.pezzottify.android.localplayer.data.LocalPlaylistWithCount
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

@HiltViewModel
class LocalPlayerViewModel @Inject constructor(
    application: Application,
    private val playlistDao: LocalPlaylistDao
) : AndroidViewModel(application) {

    private val localExoPlayer = LocalExoPlayer(application, viewModelScope)

    val state: StateFlow<LocalPlayerState> = localExoPlayer.state

    val playlists: Flow<List<LocalPlaylistWithCount>> = playlistDao.getAllPlaylistsWithCount()

    private val _hasRestoredState = MutableStateFlow(false)
    val hasRestoredState: StateFlow<Boolean> = _hasRestoredState.asStateFlow()

    /**
     * Try to restore previous playback state.
     * Returns true if state was restored, false if no saved state exists.
     */
    fun tryRestoreState(): Boolean {
        val prefs = getApplication<Application>().getSharedPreferences(
            LocalPlaybackService.PREFS_NAME,
            Context.MODE_PRIVATE
        )

        val savedAt = prefs.getLong(LocalPlaybackService.KEY_SAVED_AT, 0)
        if (savedAt == 0L) return false

        // Check if saved state is not too old (24 hours)
        val maxAge = 24 * 60 * 60 * 1000L // 24 hours in milliseconds
        if (System.currentTimeMillis() - savedAt > maxAge) {
            clearSavedState()
            return false
        }

        val queueUrisString = prefs.getString(LocalPlaybackService.KEY_QUEUE_URIS, null)
        if (queueUrisString.isNullOrEmpty()) return false

        val uriStrings = queueUrisString.split(LocalPlaybackService.SEPARATOR)
        if (uriStrings.isEmpty()) return false

        val contentResolver = getApplication<Application>().contentResolver
        val tracks = uriStrings.mapNotNull { uriString ->
            try {
                val uri = Uri.parse(uriString)
                // Check if we can still access this URI
                if (!canAccessUri(uri, contentResolver)) {
                    return@mapNotNull null
                }
                LocalTrackInfo(
                    uri = uriString,
                    displayName = getDisplayName(uri)
                )
            } catch (e: Exception) {
                null
            }
        }

        if (tracks.isEmpty()) {
            // No accessible tracks - clear saved state
            clearSavedState()
            return false
        }

        val currentIndex = prefs.getInt(LocalPlaybackService.KEY_CURRENT_INDEX, 0)
            .coerceIn(0, tracks.size - 1)
        val positionMs = prefs.getLong(LocalPlaybackService.KEY_POSITION_MS, 0)

        localExoPlayer.restoreState(tracks, currentIndex, positionMs)
        _hasRestoredState.value = true
        return true
    }

    fun clearSavedState() {
        getApplication<Application>().getSharedPreferences(
            LocalPlaybackService.PREFS_NAME,
            Context.MODE_PRIVATE
        ).edit().clear().apply()
    }

    fun loadFiles(uris: List<Uri>) {
        if (uris.isEmpty()) return

        val tracks = uris.map { uri ->
            LocalTrackInfo(
                uri = uri.toString(),
                displayName = getDisplayName(uri)
            )
        }
        localExoPlayer.loadQueue(tracks)
    }

    fun addToQueue(uris: List<Uri>) {
        if (uris.isEmpty()) return

        val tracks = uris.map { uri ->
            LocalTrackInfo(
                uri = uri.toString(),
                displayName = getDisplayName(uri)
            )
        }
        localExoPlayer.addToQueue(tracks)
    }

    fun togglePlayPause() {
        localExoPlayer.togglePlayPause()
    }

    fun seekToPercent(percent: Float) {
        localExoPlayer.seekToPercent(percent)
    }

    fun skipNext() {
        localExoPlayer.skipNext()
    }

    fun skipPrevious() {
        localExoPlayer.skipPrevious()
    }

    fun selectTrack(index: Int) {
        localExoPlayer.selectTrack(index)
    }

    override fun onCleared() {
        super.onCleared()
        localExoPlayer.release()
    }

    // Playlist methods

    fun saveQueueAsPlaylist(name: String) {
        val currentQueue = state.value.queue
        if (currentQueue.isEmpty()) return

        viewModelScope.launch {
            val playlistId = UUID.randomUUID().toString()
            val playlist = LocalPlaylistEntity(
                id = playlistId,
                name = name,
                createdAt = System.currentTimeMillis()
            )
            val tracks = currentQueue.mapIndexed { index, track ->
                LocalPlaylistTrackEntity(
                    playlistId = playlistId,
                    position = index,
                    uri = track.uri,
                    displayName = track.displayName
                )
            }
            playlistDao.insertPlaylistWithTracks(playlist, tracks)
        }
    }

    fun loadPlaylist(playlistId: String) {
        viewModelScope.launch {
            val trackEntities = playlistDao.getPlaylistTracks(playlistId)
            if (trackEntities.isEmpty()) return@launch

            val contentResolver = getApplication<Application>().contentResolver
            val tracks = trackEntities.mapNotNull { entity ->
                try {
                    val uri = Uri.parse(entity.uri)
                    // Check if we can still access this URI
                    if (!canAccessUri(uri, contentResolver)) {
                        return@mapNotNull null
                    }
                    LocalTrackInfo(
                        uri = entity.uri,
                        displayName = entity.displayName
                    )
                } catch (e: Exception) {
                    null
                }
            }

            if (tracks.isNotEmpty()) {
                localExoPlayer.loadQueue(tracks)
            }
        }
    }

    fun deletePlaylist(playlistId: String) {
        viewModelScope.launch {
            playlistDao.deletePlaylist(playlistId)
        }
    }

    private fun canAccessUri(uri: Uri, contentResolver: ContentResolver): Boolean {
        return try {
            // Try to open the URI - if it fails, we don't have access
            contentResolver.openInputStream(uri)?.use { true } ?: false
        } catch (e: Exception) {
            false
        }
    }

    private fun getDisplayName(uri: Uri): String {
        val contentResolver: ContentResolver = getApplication<Application>().contentResolver

        // Try to get display name from content resolver
        if (uri.scheme == "content") {
            try {
                contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                    if (cursor.moveToFirst()) {
                        val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                        if (nameIndex >= 0) {
                            return cursor.getString(nameIndex)
                        }
                    }
                }
            } catch (e: Exception) {
                // Fall through to default
            }
        }

        // Fallback: extract filename from path
        return uri.lastPathSegment?.substringAfterLast('/') ?: "Unknown"
    }
}

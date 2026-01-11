package com.lelloman.pezzottify.android.localplayer

import android.app.Application
import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Inject

@HiltViewModel
class LocalPlayerViewModel @Inject constructor(
    application: Application
) : AndroidViewModel(application) {

    private val localExoPlayer = LocalExoPlayer(application, viewModelScope)

    val state: StateFlow<LocalPlayerState> = localExoPlayer.state

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

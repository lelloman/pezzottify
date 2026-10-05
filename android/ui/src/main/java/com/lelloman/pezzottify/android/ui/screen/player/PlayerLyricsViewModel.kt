package com.lelloman.pezzottify.android.ui.screen.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.usecase.GetTrackLyrics
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class PlayerLyricsState(
    val trackId: String = "",
    val plainText: String? = null,
    val lines: List<LyricLine> = emptyList(),
) {
    val hasLyrics: Boolean get() = lines.isNotEmpty() || !plainText.isNullOrBlank()
}

@HiltViewModel
class PlayerLyricsViewModel @Inject constructor(private val getLyrics: GetTrackLyrics) : ViewModel() {
    private val mutableState = MutableStateFlow(PlayerLyricsState())
    val state = mutableState.asStateFlow()
    private var request: Job? = null
    private var generation = 0

    fun load(trackId: String) {
        val version = ++generation
        request?.cancel()
        mutableState.value = PlayerLyricsState(trackId)
        if (trackId.isBlank()) return
        request = viewModelScope.launch {
            try {
                val response = getLyrics(trackId)
                if (version != generation) return@launch
                val lyrics = (response as? RemoteApiResponse.Success)?.data
                if (lyrics?.status == "found" && lyrics.trackId == trackId) {
                    mutableState.value = PlayerLyricsState(trackId, lyrics.plainLyrics, parseSyncedLyrics(lyrics.syncedLyrics))
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                // Lyrics are optional: unavailable lyrics must never interrupt playback.
            }
        }
    }
}

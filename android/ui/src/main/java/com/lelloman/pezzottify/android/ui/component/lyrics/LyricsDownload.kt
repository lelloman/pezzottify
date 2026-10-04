package com.lelloman.pezzottify.android.ui.component.lyrics

import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.usecase.DownloadLyrics
import com.lelloman.pezzottify.android.ui.R
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class LyricsDownloadViewModel @Inject constructor(private val download: DownloadLyrics) : ViewModel() {
    private val pending = mutableSetOf<Pair<String, String>>()
    fun request(type: String, id: String, onResult: (Boolean) -> Unit) {
        val key = type to id
        if (!pending.add(key)) return
        viewModelScope.launch {
            try {
                onResult(download(type, id) is RemoteApiResponse.Success)
            } finally {
                pending.remove(key)
            }
        }
    }
}

/** The request outlives the dismissed menu in its screen's ViewModel. */
@Composable
fun lyricsDownloadAction(type: String, id: String): () -> Unit {
    if (LocalInspectionMode.current) return {}
    val model = hiltViewModel<LyricsDownloadViewModel>()
    val context = LocalContext.current.applicationContext
    return {
        model.request(type, id) { accepted ->
            Toast.makeText(
                context,
                if (accepted) R.string.lyrics_download_started else R.string.lyrics_download_failed,
                Toast.LENGTH_LONG,
            ).show()
        }
    }
}

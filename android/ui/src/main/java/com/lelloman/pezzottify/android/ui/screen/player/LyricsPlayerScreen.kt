package com.lelloman.pezzottify.android.ui.screen.player

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LyricsPlayerScreen(
    state: PlayerScreenState,
    lyrics: PlayerLyricsState,
    actions: PlayerScreenActions,
    onBack: () -> Unit,
) {
    BackHandler(onBack = onBack)
    Scaffold(topBar = {
        TopAppBar(
            title = {
                Column {
                    Text(state.trackName, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    state.remoteDeviceName?.let {
                        Text(stringResource(R.string.controlling_device, it),
                            style = MaterialTheme.typography.labelSmall, maxLines = 1,
                            overflow = TextOverflow.Ellipsis)
                    }
                }
            },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                        contentDescription = stringResource(R.string.player_back_to_player))
                }
            },
        )
    }) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = actions::clickOnSkipPrevious, enabled = state.hasPreviousTrack && !state.isLoading) {
                    Icon(painterResource(R.drawable.baseline_skip_previous_24), stringResource(R.string.previous))
                }
                IconButton(onClick = actions::clickOnPlayPause, enabled = !state.isLoading, modifier = Modifier.size(56.dp)) {
                    Icon(painterResource(if (state.isPlaying) R.drawable.baseline_pause_24 else R.drawable.baseline_play_arrow_24),
                        stringResource(if (state.isPlaying) R.string.pause else R.string.play), modifier = Modifier.size(36.dp))
                }
                IconButton(onClick = actions::clickOnSkipNext, enabled = state.hasNextTrack && !state.isLoading) {
                    Icon(painterResource(R.drawable.baseline_skip_next_24), stringResource(R.string.next))
                }
            }
            key(state.trackId) {
                ProgressSection(state.trackProgressPercent, state.trackProgressSec, state.trackDurationSec) {
                    if (!state.isLoading && state.trackDurationSec > 0) actions.seekToPercent(it)
                }
            }
            if (!state.isLoading && lyrics.trackId == state.trackId && lyrics.hasLyrics) {
                key(state.trackId) {
                    PlayerLyrics(lyrics, state.trackProgressSec, state.trackDurationSec,
                        modifier = Modifier.weight(1f).padding(vertical = 12.dp), expanded = true,
                        onSeek = actions::seekToPercent)
                }
            } else {
                Text(stringResource(R.string.player_lyrics_unavailable),
                    modifier = Modifier.padding(vertical = 24.dp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

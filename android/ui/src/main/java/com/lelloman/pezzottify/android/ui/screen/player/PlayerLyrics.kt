package com.lelloman.pezzottify.android.ui.screen.player

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

@Composable
internal fun PlayerLyrics(
    lyrics: PlayerLyricsState,
    positionSec: Int,
    durationSec: Int,
    modifier: Modifier = Modifier,
    expanded: Boolean = false,
    onSeek: (Float) -> Unit,
) {
    val listState = rememberLazyListState()
    var following by remember(lyrics.trackId) { mutableStateOf(true) }
    val dragged by listState.interactionSource.collectIsDraggedAsState()
    val activeIndex = activeLyricIndex(lyrics.lines, positionSec.toLong() * 1000)
    LaunchedEffect(dragged) { if (dragged) following = false }
    LaunchedEffect(lyrics.trackId, activeIndex, following) {
        if (following && activeIndex >= 0) {
            // Leave one line of context above the current line. Only this section
            // scrolls; the player page is never pulled away from its controls.
            listState.animateScrollToItem((activeIndex - 1).coerceAtLeast(0))
        }
    }
    Column(
        modifier = modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainer, RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.player_lyrics), style = MaterialTheme.typography.titleLarge)
            if (lyrics.lines.isNotEmpty() && !following) {
                TextButton(onClick = { following = true }) {
                    Text(stringResource(R.string.player_lyrics_follow))
                }
            }
        }
        Text(
            stringResource(if (lyrics.lines.isNotEmpty()) R.string.player_lyrics_synced else R.string.player_lyrics_plain),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 12.dp),
        )
        LazyColumn(state = listState, modifier = Modifier.fillMaxWidth().then(
            if (expanded) Modifier.weight(1f) else Modifier.height(360.dp)
        )) {
            if (lyrics.lines.isNotEmpty()) {
                itemsIndexed(lyrics.lines) { index, line ->
                    val active = index == activeIndex
                    val seekLabel = stringResource(R.string.player_lyrics_seek)
                    Text(
                        text = line.text.ifBlank { "♪" },
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                        color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.fillMaxWidth()
                            .background(if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainer, RoundedCornerShape(8.dp))
                            .semantics { selected = active }
                            .clickable(enabled = durationSec > 0, onClickLabel = seekLabel) {
                                onSeek((line.timeMs / (durationSec * 10f)).coerceIn(0f, 100f))
                                following = true
                            }
                            .padding(horizontal = 12.dp, vertical = 14.dp),
                    )
                }
            } else {
                item {
                    Text(lyrics.plainText.orEmpty(), style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp))
                }
            }
        }
    }
}

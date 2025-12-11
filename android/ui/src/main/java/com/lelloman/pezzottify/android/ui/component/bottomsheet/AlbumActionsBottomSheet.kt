package com.lelloman.pezzottify.android.ui.component.bottomsheet

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.content.Album

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AlbumActionsBottomSheet(
    album: Album,
    sheetState: SheetState,
    onDismiss: () -> Unit,
    onPlay: () -> Unit,
    onAddToQueue: () -> Unit,
    onAddToPlaylist: () -> Unit,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 24.dp)
        ) {
            // Album info header
            AlbumInfoHeader(album = album)

            HorizontalDivider(
                modifier = Modifier.padding(vertical = 8.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            // Actions
            AlbumActionItem(
                iconRes = R.drawable.baseline_play_arrow_24,
                label = stringResource(R.string.play),
                onClick = {
                    onPlay()
                    onDismiss()
                }
            )

            AlbumActionItem(
                iconRes = R.drawable.baseline_playlist_add_24,
                label = stringResource(R.string.add_to_queue),
                onClick = {
                    onAddToQueue()
                    onDismiss()
                }
            )

            AlbumActionItem(
                iconRes = R.drawable.baseline_queue_music_24,
                label = stringResource(R.string.add_all_tracks_to_playlist),
                onClick = {
                    onAddToPlaylist()
                }
            )
        }
    }
}

@Composable
private fun AlbumInfoHeader(album: Album) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Text(
            text = album.name,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(modifier = Modifier.height(4.dp))
        val trackCount = album.discs.sumOf { it.tracksIds.size }
        Text(
            text = pluralStringResource(R.plurals.tracks_count, trackCount, trackCount),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
    }
}

@Composable
private fun AlbumActionItem(
    iconRes: Int,
    label: String,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            painter = painterResource(iconRes),
            contentDescription = label,
            modifier = Modifier.size(24.dp),
            tint = MaterialTheme.colorScheme.onSurface
        )
        Spacer(modifier = Modifier.width(16.dp))
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}

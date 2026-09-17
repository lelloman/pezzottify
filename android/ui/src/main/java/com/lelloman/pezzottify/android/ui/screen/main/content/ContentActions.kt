package com.lelloman.pezzottify.android.ui.screen.main.content

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

@Composable
fun ContentPlaybackActions(
    isLiked: Boolean,
    onLike: () -> Unit,
    onPlay: () -> Unit,
    modifier: Modifier = Modifier,
    playEnabled: Boolean = true,
    playDescription: String = stringResource(R.string.play),
) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        FilledTonalIconButton(onClick = onLike, modifier = Modifier.size(56.dp)) {
            Icon(
                painterResource(if (isLiked) R.drawable.baseline_favorite_24 else R.drawable.baseline_favorite_border_24),
                stringResource(if (isLiked) R.string.unlike else R.string.like),
                modifier = Modifier.size(28.dp),
                tint = if (isLiked) Color.Red else LocalContentColor.current,
            )
        }
        FilledIconButton(onClick = onPlay, enabled = playEnabled, modifier = Modifier.size(56.dp)) {
            Icon(painterResource(R.drawable.baseline_play_arrow_24), playDescription, Modifier.size(28.dp))
        }
    }
}

@Composable
fun ContentMoreButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = LocalContentColor.current,
) {
    IconButton(onClick = onClick, modifier = modifier.size(48.dp)) {
        Icon(painterResource(R.drawable.baseline_more_vert_24), stringResource(R.string.more_options), tint = tint)
    }
}

@Composable
fun ContentOverflowMenu(
    modifier: Modifier = Modifier,
    tint: Color = LocalContentColor.current,
    content: @Composable ColumnScope.(dismiss: () -> Unit) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier, contentAlignment = Alignment.TopEnd) {
        ContentMoreButton(onClick = { expanded = true }, tint = tint)
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            content { expanded = false }
        }
    }
}

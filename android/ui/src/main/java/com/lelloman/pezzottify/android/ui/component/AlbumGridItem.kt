package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.content.AlbumAvailability
import com.lelloman.pezzottify.android.ui.R

@Composable
fun AlbumGridItem(
    modifier: Modifier = Modifier,
    albumName: String,
    albumDate: Int,
    albumCoverUrl: String? = null,
    availability: AlbumAvailability = AlbumAvailability.Complete,
    onClick: () -> Unit,
) {
    val isUnavailable = availability == AlbumAvailability.Missing
    val isPartial = availability == AlbumAvailability.Partial

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .let { if (isUnavailable) it.alpha(0.5f) else it }
    ) {
        Box {
            NullablePezzottifyImage(
                url = albumCoverUrl,
                shape = PezzottifyImageShape.FillWidthSquare,
                placeholder = PezzottifyImagePlaceholder.GenericImage,
            )

            // Show availability indicator for partial or missing albums
            if (isPartial || isUnavailable) {
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(4.dp)
                        .size(24.dp)
                        .background(
                            color = if (isUnavailable)
                                MaterialTheme.colorScheme.error.copy(alpha = 0.9f)
                            else
                                MaterialTheme.colorScheme.tertiary.copy(alpha = 0.9f),
                            shape = CircleShape
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Warning,
                        contentDescription = if (isUnavailable)
                            stringResource(R.string.album_unavailable)
                        else
                            stringResource(R.string.album_partial),
                        tint = Color.White,
                        modifier = Modifier.size(16.dp)
                    )
                }
            }
        }

        Text(
            text = albumName,
            style = MaterialTheme.typography.titleSmall,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp)
        )

        Text(
            text = formatYear(albumDate),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 2.dp)
        )
    }
}

private fun formatYear(yyyymmdd: Int): String {
    return if (yyyymmdd > 0) {
        val year = yyyymmdd / 10000
        year.toString()
    } else {
        ""
    }
}

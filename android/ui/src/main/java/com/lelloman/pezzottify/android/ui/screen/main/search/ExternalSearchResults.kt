package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.HourglassEmpty
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.SubcomposeAsyncImage
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.Spacing

@Composable
fun ExternalAlbumSearchResult(
    result: ExternalSearchResultContent.Album,
    canRequest: Boolean,
    isRequesting: Boolean,
    onRequestClick: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Album cover
        SubcomposeAsyncImage(
            model = result.imageUrl,
            contentDescription = result.name,
            modifier = Modifier
                .size(56.dp)
                .clip(RoundedCornerShape(4.dp)),
            contentScale = ContentScale.Crop,
        )

        Spacer(modifier = Modifier.width(Spacing.Medium))

        // Album info
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight(),
            verticalArrangement = Arrangement.Center,
        ) {
            Text(
                text = result.name,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = buildString {
                    append(result.artistName)
                    result.year?.let { append(" • $it") }
                },
                style = MaterialTheme.typography.bodySmall,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Spacer(modifier = Modifier.width(Spacing.Small))

        // Status/Action button
        ExternalResultActionButton(
            inCatalog = result.inCatalog,
            inQueue = result.inQueue,
            canRequest = canRequest,
            isRequesting = isRequesting,
            onRequestClick = onRequestClick,
        )
    }
}

@Composable
fun ExternalArtistSearchResult(
    result: ExternalSearchResultContent.Artist,
    canRequest: Boolean,
    isRequesting: Boolean,
    onRequestClick: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Artist image (circular)
        SubcomposeAsyncImage(
            model = result.imageUrl,
            contentDescription = result.name,
            modifier = Modifier
                .size(56.dp)
                .clip(CircleShape),
            contentScale = ContentScale.Crop,
        )

        Spacer(modifier = Modifier.width(Spacing.Medium))

        // Artist name
        Text(
            text = result.name,
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )

        Spacer(modifier = Modifier.width(Spacing.Small))

        // Status/Action button
        ExternalResultActionButton(
            inCatalog = result.inCatalog,
            inQueue = result.inQueue,
            canRequest = canRequest,
            isRequesting = isRequesting,
            onRequestClick = onRequestClick,
        )
    }
}

@Composable
private fun ExternalResultActionButton(
    inCatalog: Boolean,
    inQueue: Boolean,
    canRequest: Boolean,
    isRequesting: Boolean,
    onRequestClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    when {
        inCatalog -> {
            // Already in catalog - show check icon
            Icon(
                imageVector = Icons.Default.Check,
                contentDescription = stringResource(R.string.in_catalog),
                tint = MaterialTheme.colorScheme.primary,
                modifier = modifier.size(24.dp),
            )
        }
        inQueue -> {
            // In download queue - show hourglass
            Icon(
                imageVector = Icons.Default.HourglassEmpty,
                contentDescription = stringResource(R.string.in_queue),
                tint = MaterialTheme.colorScheme.secondary,
                modifier = modifier.size(24.dp),
            )
        }
        isRequesting -> {
            // Currently sending request - show loading
            CircularProgressIndicator(
                modifier = modifier.size(24.dp),
                strokeWidth = 2.dp,
            )
        }
        else -> {
            // Can request download
            Button(
                onClick = onRequestClick,
                enabled = canRequest,
                modifier = modifier.height(32.dp),
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Add,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(modifier = Modifier.width(4.dp))
                Text(
                    text = stringResource(R.string.request),
                    style = MaterialTheme.typography.labelMedium,
                )
            }
        }
    }
}

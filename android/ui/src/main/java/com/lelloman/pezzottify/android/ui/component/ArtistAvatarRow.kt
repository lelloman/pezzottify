package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver

@Composable
fun ArtistAvatarRow(
    artistIds: List<String>,
    contentResolver: ContentResolver,
    onArtistClick: (String) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp)
    ) {
        artistIds.forEach { artistId ->
            ArtistAvatarItem(
                artistId = artistId,
                contentResolver = contentResolver,
                onClick = { onArtistClick(artistId) }
            )
        }
    }
}

@Composable
private fun ArtistAvatarItem(
    artistId: String,
    contentResolver: ContentResolver,
    onClick: () -> Unit
) {
    val artistFlow = contentResolver.resolveArtist(artistId)
    val artistState = artistFlow.collectAsState(Content.Loading(artistId))

    when (val artist = artistState.value) {
        is Content.Resolved -> {
            Column(
                modifier = Modifier
                    .width(96.dp)
                    .clickable(onClick = onClick)
                    .padding(end = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                NullablePezzottifyImage(
                    url = artist.data.imageUrl,
                    shape = PezzottifyImageShape.SmallSquare,
                    placeholder = PezzottifyImagePlaceholder.Head,
                    modifier = Modifier.clip(CircleShape)
                )

                Text(
                    text = artist.data.name,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp)
                )
            }
        }
        is Content.Loading, is Content.Error -> {
            // Don't show anything for loading or error states
        }
    }
}

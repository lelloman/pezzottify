package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.MarqueeAnimationMode
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import com.lelloman.pezzottify.android.ui.content.ArtistInfo

@Composable
fun ScrollingArtistsRow(
    artists: List<ArtistInfo>,
    modifier: Modifier = Modifier,
    onArtistClick: ((artistId: String) -> Unit)? = null,
    textStyle: TextStyle = MaterialTheme.typography.bodyMedium,
    textColor: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    Row(
        modifier = modifier.basicMarquee(
            animationMode = MarqueeAnimationMode.Immediately,
            initialDelayMillis = 1000
        )
    ) {
        artists.forEachIndexed { index, artist ->
            if (artist.name.isNotEmpty()) {
                Text(
                    text = artist.name,
                    style = textStyle,
                    color = textColor,
                    maxLines = 1,
                    modifier = if (onArtistClick != null) {
                        Modifier.clickable { onArtistClick(artist.id) }
                    } else {
                        Modifier
                    }
                )
                if (index < artists.lastIndex) {
                    Text(
                        text = ", ",
                        style = textStyle,
                        color = textColor,
                        maxLines = 1
                    )
                }
            }
        }
    }
}

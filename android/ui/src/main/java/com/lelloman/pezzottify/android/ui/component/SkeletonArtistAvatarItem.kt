package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp

/**
 * Skeleton placeholder for artist avatar items in horizontal scroll rows.
 * Matches the layout dimensions of the real ArtistAvatarItem to prevent layout shifts.
 */
@Composable
fun SkeletonArtistAvatarItem(
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .padding(end = 16.dp)
            .width(96.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        // Circular image placeholder - matches SmallCircle shape (96.dp)
        Box(
            modifier = Modifier
                .size(96.dp)
                .clip(CircleShape)
                .shimmer()
        )

        // Name placeholder
        Spacer(modifier = Modifier.height(8.dp))
        Box(
            modifier = Modifier
                .width(72.dp)
                .height(14.dp)
                .clip(RoundedCornerShape(4.dp))
                .shimmer()
        )
    }
}

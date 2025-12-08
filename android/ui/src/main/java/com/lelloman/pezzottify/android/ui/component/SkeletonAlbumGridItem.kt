package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp

/**
 * Skeleton placeholder for AlbumGridItem.
 * Matches the layout dimensions of the real component to prevent layout shifts.
 */
@Composable
fun SkeletonAlbumGridItem(
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 8.dp)
    ) {
        // Image placeholder - matches FillWidthSquare shape
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(RoundedCornerShape(4.dp))
                .shimmer()
        )

        // Title placeholder
        Spacer(modifier = Modifier.height(8.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth(0.8f)
                .height(16.dp)
                .clip(RoundedCornerShape(4.dp))
                .shimmer()
        )

        // Year placeholder
        Spacer(modifier = Modifier.height(6.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth(0.3f)
                .height(12.dp)
                .clip(RoundedCornerShape(4.dp))
                .shimmer()
        )
    }
}

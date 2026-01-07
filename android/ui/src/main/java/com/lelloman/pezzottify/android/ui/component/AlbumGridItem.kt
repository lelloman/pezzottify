package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun AlbumGridItem(
    modifier: Modifier = Modifier,
    albumName: String,
    albumDate: Int,
    albumCoverUrl: String? = null,
    onClick: () -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp)
    ) {
        NullablePezzottifyImage(
            url = albumCoverUrl,
            shape = PezzottifyImageShape.FillWidthSquare,
            placeholder = PezzottifyImagePlaceholder.GenericImage,
        )

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

package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import com.lelloman.pezzottify.android.ui.theme.Spacing

@Composable
fun DownloadLimitsBar(
    limits: UiDownloadLimits,
    modifier: Modifier = Modifier,
) {
    val backgroundColor = if (limits.isAtAnyLimit) {
        MaterialTheme.colorScheme.errorContainer
    } else {
        MaterialTheme.colorScheme.surfaceVariant
    }

    val textColor = if (limits.isAtAnyLimit) {
        MaterialTheme.colorScheme.onErrorContainer
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(backgroundColor)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = buildString {
                append("${limits.requestsToday}/${limits.maxPerDay} today")
                append(" · ")
                append("${limits.inQueue}/${limits.maxQueue} in queue")
            },
            style = MaterialTheme.typography.bodySmall,
            color = textColor,
            fontWeight = if (limits.isAtAnyLimit) FontWeight.Medium else FontWeight.Normal,
        )
    }
}

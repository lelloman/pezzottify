package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

private const val MAX_BADGE_COUNT = 99

/**
 * Formats a notification count for display in a badge.
 * Shows "99+" for counts greater than 99.
 */
fun formatBadgeCount(count: Int): String {
    return if (count > MAX_BADGE_COUNT) "99+" else count.toString()
}

/**
 * A badge that displays a notification count using Material 3 BadgedBox.
 * Shows nothing if count is 0.
 */
@Composable
fun NotificationBadgedBox(
    count: Int,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    BadgedBox(
        modifier = modifier,
        badge = {
            if (count > 0) {
                Badge {
                    Text(text = formatBadgeCount(count))
                }
            }
        }
    ) {
        content()
    }
}

/**
 * A small circular badge that displays a notification count.
 * Designed for use on icons in navigation drawers.
 * Shows nothing if count is 0.
 */
@Composable
fun SmallNotificationBadge(
    count: Int,
    modifier: Modifier = Modifier,
) {
    if (count > 0) {
        Surface(
            modifier = modifier.size(16.dp),
            shape = CircleShape,
            color = MaterialTheme.colorScheme.error
        ) {
            Box(contentAlignment = Alignment.Center) {
                Text(
                    text = formatBadgeCount(count),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onError
                )
            }
        }
    }
}

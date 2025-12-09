package com.lelloman.pezzottify.android.ui.component

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.CloudSync
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Connection status indicator that shows:
 * - A rotating sync icon when connecting
 * - An error icon when connection has failed
 * - Nothing when connected or intentionally disconnected
 *
 * Note: [ConnectionState.Disconnected] represents an intentional disconnect (e.g., app in background)
 * and should not show any indicator. Only [ConnectionState.Error] represents an unexpected
 * disconnection that the user should be aware of.
 */
@Composable
fun OfflineIndicator(
    connectionState: ConnectionState,
    modifier: Modifier = Modifier,
    size: Dp = 24.dp,
) {
    AnimatedContent(
        targetState = connectionState,
        transitionSpec = { fadeIn() togetherWith fadeOut() },
        modifier = modifier,
        label = "ConnectionIndicator",
    ) { state ->
        when {
            state is ConnectionState.Connecting -> {
                RotatingSyncIcon(size = size)
            }
            state is ConnectionState.Error -> {
                Icon(
                    imageVector = Icons.Default.CloudOff,
                    contentDescription = "Offline",
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(size)
                )
            }
            else -> {
                // Empty composable for Connected/Disconnected states
                // This maintains layout space during transitions
            }
        }
    }
}

@Composable
private fun RotatingSyncIcon(
    size: Dp,
    modifier: Modifier = Modifier,
) {
    val infiniteTransition = rememberInfiniteTransition(label = "SyncRotation")
    val rotation by infiniteTransition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1500, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "SyncRotationAngle",
    )

    Icon(
        imageVector = Icons.Default.CloudSync,
        contentDescription = "Connecting",
        tint = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier
            .size(size)
            .rotate(rotation)
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewOfflineIndicatorDisconnected() {
    OfflineIndicator(
        connectionState = ConnectionState.Disconnected,
        size = 24.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewOfflineIndicatorConnecting() {
    OfflineIndicator(
        connectionState = ConnectionState.Connecting,
        size = 24.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewOfflineIndicatorConnected() {
    OfflineIndicator(
        connectionState = ConnectionState.Connected(deviceId = 1, serverVersion = "0.5.0"),
        size = 24.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewOfflineIndicatorError() {
    OfflineIndicator(
        connectionState = ConnectionState.Error("Connection failed"),
        size = 24.dp
    )
}

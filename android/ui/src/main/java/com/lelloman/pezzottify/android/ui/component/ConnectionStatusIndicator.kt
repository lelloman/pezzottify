package com.lelloman.pezzottify.android.ui.component

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState

private val ConnectedColor = Color(0xFF22c55e)   // Green
private val ConnectingColor = Color(0xFFf97316)  // Orange
private val DisconnectedColor = Color(0xFFef4444) // Red

@Composable
fun ConnectionStatusIndicator(
    connectionState: ConnectionState,
    modifier: Modifier = Modifier,
    size: Dp = 8.dp,
) {
    val color = when (connectionState) {
        is ConnectionState.Connected -> ConnectedColor
        is ConnectionState.Connecting -> ConnectingColor
        is ConnectionState.Disconnected -> DisconnectedColor
        is ConnectionState.Error -> DisconnectedColor
    }

    val alpha = if (connectionState is ConnectionState.Connecting) {
        // Pulsing animation for connecting state
        val infiniteTransition = rememberInfiniteTransition(label = "pulse")
        val pulseAlpha by infiniteTransition.animateFloat(
            initialValue = 0.3f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(
                animation = tween(800, easing = LinearEasing),
                repeatMode = RepeatMode.Reverse
            ),
            label = "pulseAlpha"
        )
        pulseAlpha
    } else {
        1f
    }

    Box(
        modifier = modifier
            .size(size)
            .alpha(alpha)
            .background(color = color, shape = CircleShape)
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewConnectionStatusConnected() {
    ConnectionStatusIndicator(
        connectionState = ConnectionState.Connected(deviceId = 1),
        size = 12.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewConnectionStatusConnecting() {
    ConnectionStatusIndicator(
        connectionState = ConnectionState.Connecting,
        size = 12.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewConnectionStatusDisconnected() {
    ConnectionStatusIndicator(
        connectionState = ConnectionState.Disconnected,
        size = 12.dp
    )
}

@Preview(showBackground = true, backgroundColor = 0xFFFFFFFF)
@Composable
private fun PreviewConnectionStatusError() {
    ConnectionStatusIndicator(
        connectionState = ConnectionState.Error("Connection failed"),
        size = 12.dp
    )
}

package com.lelloman.pezzottify.android.ui.component

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState

/**
 * Offline indicator that only shows when disconnected.
 * Hidden when connected or connecting.
 */
@Composable
fun OfflineIndicator(
    connectionState: ConnectionState,
    modifier: Modifier = Modifier,
    size: Dp = 24.dp,
) {
    val isOffline = connectionState is ConnectionState.Disconnected ||
            connectionState is ConnectionState.Error

    AnimatedVisibility(
        visible = isOffline,
        enter = fadeIn(),
        exit = fadeOut(),
        modifier = modifier,
    ) {
        Icon(
            imageVector = Icons.Default.CloudOff,
            contentDescription = "Offline",
            tint = MaterialTheme.colorScheme.error,
            modifier = Modifier.size(size)
        )
    }
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

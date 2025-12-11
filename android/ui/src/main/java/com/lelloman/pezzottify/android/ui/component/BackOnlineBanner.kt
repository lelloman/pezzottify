package com.lelloman.pezzottify.android.ui.component

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import kotlinx.coroutines.delay

private val BackOnlineColor = Color(0xFF22c55e) // Green
private const val BANNER_DISPLAY_DURATION_MS = 3000L

/**
 * A banner that slides in from the top when connection is restored after an error.
 * Auto-dismisses after a few seconds.
 *
 * Note: This banner only shows when recovering from [ConnectionState.Error], not from
 * [ConnectionState.Disconnected]. Disconnected represents an intentional disconnect
 * (e.g., app going to background) and should not trigger the "back online" banner.
 *
 * @param connectionState Current connection state
 * @param modifier Modifier for the banner
 * @param onDismissed Callback when banner is dismissed (optional)
 */
@Composable
fun BackOnlineBanner(
    connectionState: ConnectionState,
    modifier: Modifier = Modifier,
    onDismissed: () -> Unit = {},
) {
    var showBanner by remember { mutableStateOf(false) }
    var hadError by remember { mutableStateOf(false) }

    // Track connection state changes
    LaunchedEffect(connectionState) {
        val hasError = connectionState is ConnectionState.Error
        val isConnected = connectionState is ConnectionState.Connected

        if (hasError) {
            hadError = true
        } else if (isConnected && hadError) {
            // Just recovered from an error
            showBanner = true
            hadError = false
            delay(BANNER_DISPLAY_DURATION_MS)
            showBanner = false
            onDismissed()
        }
    }

    AnimatedVisibility(
        visible = showBanner,
        enter = expandVertically(expandFrom = Alignment.Top),
        exit = shrinkVertically(shrinkTowards = Alignment.Top),
        modifier = modifier,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .background(BackOnlineColor)
                .padding(vertical = 8.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = stringResource(R.string.back_online),
                style = MaterialTheme.typography.bodyMedium,
                color = Color.White,
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewBackOnlineBanner() {
    // For preview, we simulate the banner being visible
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(BackOnlineColor)
            .padding(vertical = 8.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = stringResource(R.string.back_online),
            style = MaterialTheme.typography.bodyMedium,
            color = Color.White,
        )
    }
}


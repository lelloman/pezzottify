package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.outlined.Devices
import androidx.compose.material.icons.outlined.Handshake
import androidx.compose.material.icons.outlined.LinkOff
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.PowerSettingsNew
import androidx.compose.material.icons.outlined.WifiTethering
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.session.SessionState
import com.lelloman.androidoscopy.ui.SessionActivity
import com.lelloman.androidoscopy.ui.SessionPalette
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

/** Navigation, not a persisted setting: only the SDK owns session activation/expiry. */
@Composable
fun DiagnosticSessionButton(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val session by Androidoscopy.sessionState.collectAsStateWithLifecycle()
    val palette = SessionPalette.fromColorScheme(MaterialTheme.colorScheme)
    DiagnosticSessionCard(
        active = session.active,
        remainingMinutes = session.remainingMs?.let { ((it + 59_999) / 60_000).coerceAtLeast(0) },
        onClick = { SessionActivity.launch(context, palette) },
        modifier = modifier,
        status = diagnosticSessionStatus(session),
        acceptAll = session.acceptAll,
        connectionMessage = session.reason.takeIf { session.active && session.peer == null && session.pairing == null },
    )
}

@Composable
internal fun DiagnosticSessionCard(
    active: Boolean,
    remainingMinutes: Long?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    status: DiagnosticSessionStatus = if (active) DiagnosticSessionStatus.WAITING else DiagnosticSessionStatus.OFF,
    acceptAll: Boolean = false,
    connectionMessage: String? = null,
) {
    val colors = MaterialTheme.colorScheme
    val (icon, container, foreground) = when (status) {
        DiagnosticSessionStatus.OFF -> Triple(Icons.Outlined.PowerSettingsNew, colors.surfaceContainerHighest, colors.onSurfaceVariant)
        DiagnosticSessionStatus.WAITING -> Triple(Icons.Outlined.WifiTethering, colors.secondaryContainer, colors.onSecondaryContainer)
        DiagnosticSessionStatus.PAIRING -> Triple(Icons.Outlined.Handshake, colors.tertiaryContainer, colors.onTertiaryContainer)
        DiagnosticSessionStatus.CONNECTED -> Triple(Icons.Outlined.Devices, colors.primaryContainer, colors.onPrimaryContainer)
        DiagnosticSessionStatus.ATTENTION -> Triple(Icons.Outlined.LinkOff, colors.errorContainer, colors.onErrorContainer)
    }
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Surface(shape = RoundedCornerShape(12.dp), color = container) {
                    Icon(icon, contentDescription = null, modifier = Modifier.padding(12.dp).size(24.dp), tint = foreground)
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(stringResource(R.string.diagnostic_session), style = MaterialTheme.typography.titleMedium)
                    Text(
                        stringResource(when (status) {
                            DiagnosticSessionStatus.OFF -> R.string.diagnostic_session_inactive
                            DiagnosticSessionStatus.WAITING -> R.string.diagnostic_session_waiting
                            DiagnosticSessionStatus.PAIRING -> R.string.diagnostic_session_pairing
                            DiagnosticSessionStatus.CONNECTED -> R.string.diagnostic_session_connected
                            DiagnosticSessionStatus.ATTENTION -> R.string.diagnostic_session_disconnected
                        }),
                        style = MaterialTheme.typography.labelMedium,
                        color = when (status) {
                            DiagnosticSessionStatus.OFF -> colors.onSurfaceVariant
                            DiagnosticSessionStatus.WAITING -> colors.secondary
                            DiagnosticSessionStatus.PAIRING -> colors.tertiary
                            DiagnosticSessionStatus.CONNECTED -> colors.primary
                            DiagnosticSessionStatus.ATTENTION -> colors.error
                        },
                    )
                    if (active && remainingMinutes != null) Text(
                        stringResource(R.string.diagnostic_session_remaining, remainingMinutes),
                        style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant,
                    )
                }
            }
            connectionMessage?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = colors.error) }
            if (active && acceptAll) Text(stringResource(R.string.diagnostic_session_accept_all),
                style = MaterialTheme.typography.labelMedium, color = colors.error)
            Text(stringResource(R.string.diagnostic_session_description), style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            FilledTonalButton(onClick = onClick, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(if (active) R.string.manage_diagnostic_session else R.string.open_diagnostic_session),
                    modifier = Modifier.weight(1f))
                Spacer(Modifier.width(12.dp))
                Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = null, modifier = Modifier.size(18.dp))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Outlined.Lock, contentDescription = null, modifier = Modifier.size(16.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(stringResource(R.string.diagnostic_session_privacy), style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

internal enum class DiagnosticSessionStatus { OFF, WAITING, PAIRING, CONNECTED, ATTENTION }

internal fun diagnosticSessionStatus(state: SessionState): DiagnosticSessionStatus = when {
    !state.active -> DiagnosticSessionStatus.OFF
    state.peer != null -> DiagnosticSessionStatus.CONNECTED
    state.pairing != null -> DiagnosticSessionStatus.PAIRING
    state.reason != null -> DiagnosticSessionStatus.ATTENTION
    else -> DiagnosticSessionStatus.WAITING
}

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionInactivePreview() {
    PezzottifyTheme(darkTheme = false) { DiagnosticSessionCard(false, null, {}) }
}

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionActivePreview() {
    PezzottifyTheme(darkTheme = true) { DiagnosticSessionCard(true, 12, {}, status = DiagnosticSessionStatus.CONNECTED) }
}

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionPairingPreview() {
    PezzottifyTheme(darkTheme = true) { DiagnosticSessionCard(true, 12, {}, status = DiagnosticSessionStatus.PAIRING) }
}

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionWaitingPreview() {
    PezzottifyTheme(darkTheme = false) { DiagnosticSessionCard(true, 12, {}, acceptAll = true) }
}

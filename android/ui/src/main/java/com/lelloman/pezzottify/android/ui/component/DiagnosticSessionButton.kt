package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.outlined.BugReport
import androidx.compose.material.icons.outlined.Lock
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
    )
}

@Composable
internal fun DiagnosticSessionCard(
    active: Boolean,
    remainingMinutes: Long?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Surface(shape = RoundedCornerShape(12.dp), color = MaterialTheme.colorScheme.secondaryContainer) {
                    Icon(Icons.Outlined.BugReport, contentDescription = null, modifier = Modifier.padding(12.dp).size(24.dp),
                        tint = MaterialTheme.colorScheme.onSecondaryContainer)
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(stringResource(R.string.diagnostic_session), style = MaterialTheme.typography.titleMedium)
                    Text(
                        when {
                            !active -> stringResource(R.string.diagnostic_session_inactive)
                            remainingMinutes != null -> stringResource(R.string.diagnostic_session_remaining, remainingMinutes)
                            else -> stringResource(R.string.diagnostic_session_active)
                        },
                        style = MaterialTheme.typography.labelMedium,
                        color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
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

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionInactivePreview() {
    PezzottifyTheme(darkTheme = false) { DiagnosticSessionCard(false, null, {}) }
}

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun DiagnosticSessionActivePreview() {
    PezzottifyTheme(darkTheme = true) { DiagnosticSessionCard(true, 12, {}) }
}

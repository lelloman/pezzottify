package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun FileLoggingSection(
    isEnabled: Boolean,
    hasLogs: Boolean,
    logSize: String,
    onEnabledChanged: (Boolean) -> Unit,
    onViewLogs: () -> Unit,
    onShareLogs: () -> Unit,
    onClearLogs: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        Text(
            text = "Logging",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Save logs to file",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = if (hasLogs) "Size: $logSize" else "Logs will be saved when enabled",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Switch(
                checked = isEnabled,
                onCheckedChange = onEnabledChanged
            )
        }

        if (hasLogs) {
            Spacer(modifier = Modifier.height(16.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                OutlinedButton(
                    onClick = onViewLogs,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.List,
                        contentDescription = "View logs"
                    )
                }

                OutlinedButton(
                    onClick = onShareLogs,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Default.Share,
                        contentDescription = "Share logs"
                    )
                }

                OutlinedButton(
                    onClick = onClearLogs,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Default.Delete,
                        contentDescription = "Clear logs"
                    )
                }
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun FileLoggingSectionPreviewDisabled() {
    PezzottifyTheme {
        FileLoggingSection(
            isEnabled = false,
            hasLogs = false,
            logSize = "",
            onEnabledChanged = {},
            onViewLogs = {},
            onShareLogs = {},
            onClearLogs = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun FileLoggingSectionPreviewEnabledNoLogs() {
    PezzottifyTheme {
        FileLoggingSection(
            isEnabled = true,
            hasLogs = false,
            logSize = "",
            onEnabledChanged = {},
            onViewLogs = {},
            onShareLogs = {},
            onClearLogs = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun FileLoggingSectionPreviewEnabledWithLogs() {
    PezzottifyTheme {
        FileLoggingSection(
            isEnabled = true,
            hasLogs = true,
            logSize = "2.3 MB",
            onEnabledChanged = {},
            onViewLogs = {},
            onShareLogs = {},
            onClearLogs = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun FileLoggingSectionPreviewDark() {
    PezzottifyTheme(darkTheme = true) {
        FileLoggingSection(
            isEnabled = true,
            hasLogs = true,
            logSize = "1.5 MB",
            onEnabledChanged = {},
            onViewLogs = {},
            onShareLogs = {},
            onClearLogs = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

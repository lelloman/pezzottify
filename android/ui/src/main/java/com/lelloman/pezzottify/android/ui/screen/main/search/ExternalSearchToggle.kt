package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CloudDownload
import androidx.compose.material.icons.outlined.CloudDownload
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun ExternalSearchToggle(
    isExternalMode: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    IconButton(
        onClick = onToggle,
        modifier = modifier,
    ) {
        Icon(
            imageVector = if (isExternalMode) {
                Icons.Filled.CloudDownload
            } else {
                Icons.Outlined.CloudDownload
            },
            contentDescription = if (isExternalMode) {
                "Switch to catalog search"
            } else {
                "Switch to external search"
            },
            tint = if (isExternalMode) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            modifier = Modifier.size(24.dp)
        )
    }
}

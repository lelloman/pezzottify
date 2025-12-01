package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun CacheSettingsSection(
    isCacheEnabled: Boolean,
    onCacheEnabledChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        Text(
            text = "Performance",
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
                    text = "In-memory cache",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = "Cache content in memory for faster loading",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Switch(
                checked = isCacheEnabled,
                onCheckedChange = onCacheEnabledChanged
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun CacheSettingsSectionPreviewEnabled() {
    PezzottifyTheme {
        CacheSettingsSection(
            isCacheEnabled = true,
            onCacheEnabledChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CacheSettingsSectionPreviewDisabled() {
    PezzottifyTheme {
        CacheSettingsSection(
            isCacheEnabled = false,
            onCacheEnabledChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun ExternalSearchSection(
    isEnabled: Boolean,
    hasPermission: Boolean,
    onEnabledChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    AnimatedVisibility(
        visible = hasPermission,
        enter = fadeIn() + expandVertically(),
        exit = fadeOut() + shrinkVertically(),
        modifier = modifier
    ) {
        Column {
            Text(
                text = stringResource(R.string.external_search),
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
                        text = stringResource(R.string.enable_external_search),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = stringResource(R.string.external_search_description),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                Switch(
                    checked = isEnabled,
                    onCheckedChange = onEnabledChanged
                )
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun ExternalSearchSectionPreviewEnabled() {
    PezzottifyTheme {
        ExternalSearchSection(
            isEnabled = true,
            hasPermission = true,
            onEnabledChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ExternalSearchSectionPreviewDisabled() {
    PezzottifyTheme {
        ExternalSearchSection(
            isEnabled = false,
            hasPermission = true,
            onEnabledChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ExternalSearchSectionPreviewNoPermission() {
    PezzottifyTheme {
        ExternalSearchSection(
            isEnabled = false,
            hasPermission = false,
            onEnabledChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

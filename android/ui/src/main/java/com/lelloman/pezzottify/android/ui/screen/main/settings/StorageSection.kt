package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlin.math.roundToInt

@Composable
fun StorageSection(
    storageInfo: StorageInfo?,
    staticsCacheSizeBytes: Long?,
    imageCacheSizeBytes: Long?,
    isTrimStaticsInProgress: Boolean,
    isTrimImageInProgress: Boolean,
    isClearStaticsInProgress: Boolean,
    isClearImageInProgress: Boolean,
    onTrimStatics: () -> Unit,
    onTrimImage: () -> Unit,
    onClearStatics: () -> Unit,
    onClearImage: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        Text(
            text = stringResource(R.string.storage),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Device storage info
        if (storageInfo != null) {
            val usedGB = storageInfo.usedBytes / (1024.0 * 1024.0 * 1024.0)
            val totalGB = storageInfo.totalBytes / (1024.0 * 1024.0 * 1024.0)
            val availableGB = storageInfo.availableBytes / (1024.0 * 1024.0 * 1024.0)

            Column {
                Row(
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = stringResource(R.string.storage_used, String.format("%.2f", usedGB), String.format("%.2f", totalGB)),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }

                Spacer(modifier = Modifier.height(8.dp))

                LinearProgressIndicator(
                    progress = { storageInfo.usedPercentage.toFloat() },
                    modifier = Modifier.fillMaxWidth(),
                    color = when (storageInfo.pressureLevel) {
                        StoragePressureLevel.LOW -> MaterialTheme.colorScheme.primary
                        StoragePressureLevel.MEDIUM -> MaterialTheme.colorScheme.tertiary
                        StoragePressureLevel.HIGH -> MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
                        StoragePressureLevel.CRITICAL -> MaterialTheme.colorScheme.error
                    }
                )

                Spacer(modifier = Modifier.height(8.dp))

                Row(
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = stringResource(R.string.storage_available, String.format("%.2f", availableGB)),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.weight(1f))
                    Text(
                        text = when (storageInfo.pressureLevel) {
                            StoragePressureLevel.LOW -> stringResource(R.string.storage_plenty)
                            StoragePressureLevel.MEDIUM -> stringResource(R.string.storage_moderate)
                            StoragePressureLevel.HIGH -> stringResource(R.string.storage_low)
                            StoragePressureLevel.CRITICAL -> stringResource(R.string.storage_critical)
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = when (storageInfo.pressureLevel) {
                            StoragePressureLevel.LOW -> MaterialTheme.colorScheme.primary
                            StoragePressureLevel.MEDIUM -> MaterialTheme.colorScheme.tertiary
                            StoragePressureLevel.HIGH -> MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
                            StoragePressureLevel.CRITICAL -> MaterialTheme.colorScheme.error
                        }
                    )
                }
            }
        } else {
            Text(
                text = stringResource(R.string.storage_unavailable),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        // Catalog Cache Row
        CacheRow(
            label = stringResource(R.string.catalog_cache),
            description = stringResource(R.string.catalog_cache_description),
            sizeBytes = staticsCacheSizeBytes,
            isTrimInProgress = isTrimStaticsInProgress,
            isClearInProgress = isClearStaticsInProgress,
            onTrim = onTrimStatics,
            onClear = onClearStatics,
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Image Cache Row
        CacheRow(
            label = stringResource(R.string.image_cache),
            description = stringResource(R.string.image_cache_description),
            sizeBytes = imageCacheSizeBytes,
            isTrimInProgress = isTrimImageInProgress,
            isClearInProgress = isClearImageInProgress,
            onTrim = onTrimImage,
            onClear = onClearImage,
        )
    }
}

@Composable
private fun CacheRow(
    label: String,
    description: String,
    sizeBytes: Long?,
    isTrimInProgress: Boolean,
    isClearInProgress: Boolean,
    onTrim: () -> Unit,
    onClear: () -> Unit,
) {
    Column {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = if (sizeBytes != null) {
                        stringResource(R.string.cache_size_format, formatBytes(sizeBytes))
                    } else {
                        stringResource(R.string.cache_size_loading)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedButton(
                onClick = onTrim,
                enabled = sizeBytes != null && sizeBytes > 0 && !isTrimInProgress && !isClearInProgress,
                modifier = Modifier.weight(1f)
            ) {
                if (isTrimInProgress) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp
                    )
                } else {
                    Text(stringResource(R.string.trim_cache))
                }
            }

            OutlinedButton(
                onClick = onClear,
                enabled = sizeBytes != null && sizeBytes > 0 && !isTrimInProgress && !isClearInProgress,
                modifier = Modifier.weight(1f)
            ) {
                if (isClearInProgress) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp
                    )
                } else {
                    Icon(
                        imageVector = Icons.Default.Delete,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                    Text(
                        text = stringResource(R.string.clear_cache),
                        modifier = Modifier.padding(start = 4.dp)
                    )
                }
            }
        }
    }
}

private fun formatBytes(bytes: Long): String {
    return when {
        bytes < 1024 -> "$bytes B"
        bytes < 1024 * 1024 -> "%.1f KB".format(bytes / 1024.0)
        bytes < 1024L * 1024 * 1024 -> "%.1f MB".format(bytes / (1024.0 * 1024.0))
        else -> "%.2f GB".format(bytes / (1024.0 * 1024.0 * 1024.0))
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageSectionPreview() {
    PezzottifyTheme {
        StorageSection(
            storageInfo = StorageInfo(
                totalBytes = 128L * 1024L * 1024L * 1024L,
                availableBytes = 100L * 1024L * 1024L * 1024L,
                usedBytes = 28L * 1024L * 1024L * 1024L,
                pressureLevel = StoragePressureLevel.LOW
            ),
            staticsCacheSizeBytes = 1024 * 1024 * 5,
            imageCacheSizeBytes = 1024 * 1024 * 25,
            isTrimStaticsInProgress = false,
            isTrimImageInProgress = false,
            isClearStaticsInProgress = false,
            isClearImageInProgress = false,
            onTrimStatics = {},
            onTrimImage = {},
            onClearStatics = {},
            onClearImage = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

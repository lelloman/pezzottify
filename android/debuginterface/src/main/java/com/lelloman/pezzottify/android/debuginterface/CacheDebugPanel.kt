package com.lelloman.pezzottify.android.debuginterface

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.cache.CacheMetrics
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollector
import com.lelloman.pezzottify.android.domain.cache.CachePerformanceReport
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.memory.MemoryInfo
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureLevel
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor

@Composable
fun CacheDebugPanel(
    memoryPressureMonitor: MemoryPressureMonitor,
    staticsCache: StaticsCache,
    cacheMetricsCollector: CacheMetricsCollector,
    modifier: Modifier = Modifier
) {
    val memoryInfo by memoryPressureMonitor.memoryInfo.collectAsState()
    var cacheMetrics by remember { mutableStateOf(staticsCache.getAllMetrics()) }
    var performanceReport by remember { mutableStateOf(cacheMetricsCollector.getReport()) }

    Column(modifier = modifier.padding(16.dp)) {
        Text(
            text = "Cache Debug Panel",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Memory Info Section
        MemoryInfoSection(memoryInfo = memoryInfo, onRefresh = {
            memoryPressureMonitor.refresh()
        })

        Spacer(modifier = Modifier.height(16.dp))

        // Cache Statistics Section
        CacheStatisticsSection(
            metrics = cacheMetrics,
            onRefresh = {
                cacheMetrics = staticsCache.getAllMetrics()
            }
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Performance Report Section
        PerformanceReportSection(
            report = performanceReport,
            onRefresh = {
                performanceReport = cacheMetricsCollector.getReport()
            }
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Actions
        ActionsSection(
            onClearCache = {
                staticsCache.clearAll()
                cacheMetrics = staticsCache.getAllMetrics()
            },
            onResetMetrics = {
                staticsCache.resetAllMetrics()
                cacheMetricsCollector.reset()
                cacheMetrics = staticsCache.getAllMetrics()
                performanceReport = cacheMetricsCollector.getReport()
            }
        )
    }
}

@Composable
private fun MemoryInfoSection(
    memoryInfo: MemoryInfo,
    onRefresh: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Memory Status",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold
                )
                OutlinedButton(onClick = onRefresh) {
                    Text("Refresh")
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Pressure Level
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text("Pressure Level:")
                PressureLevelBadge(level = memoryInfo.pressureLevel)
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Memory usage bar
            val usagePercent = if (memoryInfo.maxHeapBytes > 0) {
                memoryInfo.usedBytes.toFloat() / memoryInfo.maxHeapBytes
            } else 0f

            Text(
                text = "Heap Usage: ${formatBytes(memoryInfo.usedBytes)} / ${formatBytes(memoryInfo.maxHeapBytes)}",
                style = MaterialTheme.typography.bodySmall
            )
            LinearProgressIndicator(
                progress = { usagePercent },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 4.dp),
                color = when {
                    usagePercent > 0.85f -> MaterialTheme.colorScheme.error
                    usagePercent > 0.7f -> Color(0xFFFF9800)
                    else -> MaterialTheme.colorScheme.primary
                }
            )

            Text(
                text = "Available: ${formatBytes(memoryInfo.availableBytes)}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun PressureLevelBadge(level: MemoryPressureLevel) {
    val (color, text) = when (level) {
        MemoryPressureLevel.LOW -> MaterialTheme.colorScheme.primary to "LOW"
        MemoryPressureLevel.MEDIUM -> Color(0xFFFF9800) to "MEDIUM"
        MemoryPressureLevel.HIGH -> MaterialTheme.colorScheme.error to "HIGH"
        MemoryPressureLevel.CRITICAL -> Color(0xFFB71C1C) to "CRITICAL"
    }
    Text(
        text = text,
        color = color,
        fontWeight = FontWeight.Bold,
        style = MaterialTheme.typography.bodyMedium
    )
}

@Composable
private fun CacheStatisticsSection(
    metrics: Map<String, CacheMetrics>,
    onRefresh: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Cache Statistics",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold
                )
                OutlinedButton(onClick = onRefresh) {
                    Text("Refresh")
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            metrics.forEach { (type, cacheMetrics) ->
                CacheTypeRow(type = type, metrics = cacheMetrics)
                if (type != metrics.keys.last()) {
                    HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))
                }
            }
        }
    }
}

@Composable
private fun CacheTypeRow(type: String, metrics: CacheMetrics) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = type.replaceFirstChar { it.uppercase() },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Text(
                    text = "Entries: ${metrics.currentEntries}",
                    style = MaterialTheme.typography.bodySmall
                )
                Text(
                    text = "Size: ${formatBytes(metrics.currentSizeBytes)}",
                    style = MaterialTheme.typography.bodySmall
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    text = "Hits: ${metrics.hits} / Misses: ${metrics.misses}",
                    style = MaterialTheme.typography.bodySmall
                )
                Text(
                    text = "Hit Rate: ${String.format("%.1f", metrics.hitRate * 100)}%",
                    style = MaterialTheme.typography.bodySmall,
                    color = when {
                        metrics.hitRate > 0.7 -> Color(0xFF4CAF50)
                        metrics.hitRate > 0.4 -> Color(0xFFFF9800)
                        else -> MaterialTheme.colorScheme.error
                    }
                )
            }
        }
        Text(
            text = "Evictions: ${metrics.evictions} | Expirations: ${metrics.expirations}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
private fun PerformanceReportSection(
    report: CachePerformanceReport,
    onRefresh: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Performance Metrics",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold
                )
                OutlinedButton(onClick = onRefresh) {
                    Text("Refresh")
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Time saved
            Text(
                text = "Estimated Time Saved: ${formatNanos(report.estimatedTimeSavedNanos)}",
                style = MaterialTheme.typography.bodyMedium,
                color = Color(0xFF4CAF50),
                fontWeight = FontWeight.SemiBold
            )

            Spacer(modifier = Modifier.height(8.dp))

            // Latency comparison
            report.avgCacheLatencyNanos.forEach { (type, cacheLatency) ->
                val dbLatency = report.avgDbLatencyNanos[type] ?: 0.0
                if (cacheLatency > 0 || dbLatency > 0) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            text = "${type.replaceFirstChar { it.uppercase() }}:",
                            style = MaterialTheme.typography.bodySmall
                        )
                        Text(
                            text = "Cache: ${formatNanos(cacheLatency.toLong())} | DB: ${formatNanos(dbLatency.toLong())}",
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ActionsSection(
    onClearCache: () -> Unit,
    onResetMetrics: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Button(
            onClick = onClearCache,
            modifier = Modifier.weight(1f),
            colors = ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.error
            )
        ) {
            Text("Clear Cache")
        }

        OutlinedButton(
            onClick = onResetMetrics,
            modifier = Modifier.weight(1f)
        ) {
            Text("Reset Metrics")
        }
    }
}

private fun formatBytes(bytes: Long): String {
    return when {
        bytes >= 1024 * 1024 -> String.format("%.1f MB", bytes / (1024.0 * 1024.0))
        bytes >= 1024 -> String.format("%.1f KB", bytes / 1024.0)
        else -> "$bytes B"
    }
}

private fun formatNanos(nanos: Long): String {
    return when {
        nanos >= 1_000_000_000 -> String.format("%.2f s", nanos / 1_000_000_000.0)
        nanos >= 1_000_000 -> String.format("%.2f ms", nanos / 1_000_000.0)
        nanos >= 1_000 -> String.format("%.2f us", nanos / 1_000.0)
        else -> "$nanos ns"
    }
}

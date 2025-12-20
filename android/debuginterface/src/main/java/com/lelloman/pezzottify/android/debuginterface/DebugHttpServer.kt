package com.lelloman.pezzottify.android.debuginterface

import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.cache.CacheMetrics
import com.lelloman.pezzottify.android.domain.cache.CacheMetricsCollector
import com.lelloman.pezzottify.android.domain.cache.CachePerformanceReport
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.memory.MemoryInfo
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureLevel
import com.lelloman.pezzottify.android.domain.memory.MemoryPressureMonitor
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import fi.iki.elonen.NanoHTTPD
import kotlinx.coroutines.runBlocking
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DebugHttpServer @Inject constructor(
    private val memoryPressureMonitor: MemoryPressureMonitor,
    private val staticsCache: StaticsCache,
    private val cacheMetricsCollector: CacheMetricsCollector,
    private val staticsStore: StaticsStore,
    private val systemNotificationHelper: SystemNotificationHelper,
    private val tokenRefresher: TokenRefresher,
) : NanoHTTPD(DEFAULT_PORT) {

    companion object {
        const val DEFAULT_PORT = 8088
    }

    override fun serve(session: IHTTPSession): Response {
        return when {
            session.method == Method.GET && session.uri == "/" -> serveDashboard()
            session.method == Method.POST && session.uri == "/action/clear-cache" -> handleClearCache()
            session.method == Method.POST && session.uri == "/action/clear-statics-db" -> handleClearStaticsDb()
            session.method == Method.POST && session.uri == "/action/reset-metrics" -> handleResetMetrics()
            session.method == Method.POST && session.uri == "/action/refresh-memory" -> handleRefreshMemory()
            session.method == Method.POST && session.uri == "/action/test-whatsnew-notification" -> handleTestWhatsNewNotification()
            session.method == Method.POST && session.uri == "/action/force-token-refresh" -> handleForceTokenRefresh()
            else -> newFixedLengthResponse(Response.Status.NOT_FOUND, MIME_PLAINTEXT, "Not Found")
        }
    }

    private fun serveDashboard(): Response {
        val memoryInfo = memoryPressureMonitor.memoryInfo.value
        val cacheMetrics = staticsCache.getAllMetrics()
        val performanceReport = cacheMetricsCollector.getReport()

        val html = buildDashboardHtml(memoryInfo, cacheMetrics, performanceReport)
        return newFixedLengthResponse(Response.Status.OK, "text/html", html)
    }

    private fun handleClearCache(): Response {
        staticsCache.clearAll()
        return redirectToDashboard()
    }

    private fun handleClearStaticsDb(): Response {
        runBlocking { staticsStore.deleteAll() }
        return redirectToDashboard()
    }

    private fun handleResetMetrics(): Response {
        staticsCache.resetAllMetrics()
        cacheMetricsCollector.reset()
        return redirectToDashboard()
    }

    private fun handleRefreshMemory(): Response {
        memoryPressureMonitor.refresh()
        return redirectToDashboard()
    }

    private fun handleTestWhatsNewNotification(): Response {
        systemNotificationHelper.showWhatsNewNotification(
            batchId = "test-batch-${System.currentTimeMillis()}",
            batchName = "Test Batch",
            description = "This is a test notification from the debug dashboard",
            albumsAdded = 5,
            artistsAdded = 2,
            tracksAdded = 42,
        )
        return redirectToDashboard()
    }

    private fun handleForceTokenRefresh(): Response {
        val result = runBlocking { tokenRefresher.refreshTokens() }
        val message = when (result) {
            is TokenRefresher.RefreshResult.Success -> "Token refreshed successfully"
            is TokenRefresher.RefreshResult.Failed -> "Refresh failed: ${result.reason}"
            TokenRefresher.RefreshResult.NotAvailable -> "No refresh token available"
        }
        return newFixedLengthResponse(Response.Status.OK, "text/html", """
            <!DOCTYPE html>
            <html>
            <head><meta http-equiv="refresh" content="2;url=/"></head>
            <body style="background:#1a1a2e;color:#eaeaea;font-family:sans-serif;padding:20px;">
                <h2>Token Refresh Result</h2>
                <p>$message</p>
                <p><a href="/" style="color:#00d4ff;">Back to dashboard</a></p>
            </body>
            </html>
        """.trimIndent())
    }

    private fun redirectToDashboard(): Response {
        val response = newFixedLengthResponse(Response.Status.REDIRECT, MIME_PLAINTEXT, "")
        response.addHeader("Location", "/")
        return response
    }

    private fun buildDashboardHtml(
        memoryInfo: MemoryInfo,
        cacheMetrics: Map<String, CacheMetrics>,
        performanceReport: CachePerformanceReport
    ): String {
        return """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Pezzottify Debug Dashboard</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background-color: #1a1a2e;
            color: #eaeaea;
            padding: 20px;
            line-height: 1.6;
        }
        h1 {
            color: #00d4ff;
            margin-bottom: 20px;
            font-size: 1.8em;
        }
        h2 {
            color: #00d4ff;
            margin-bottom: 12px;
            font-size: 1.2em;
            border-bottom: 1px solid #333;
            padding-bottom: 8px;
        }
        .card {
            background-color: #16213e;
            border-radius: 8px;
            padding: 16px;
            margin-bottom: 16px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.3);
        }
        .row {
            display: flex;
            justify-content: space-between;
            margin-bottom: 8px;
            flex-wrap: wrap;
        }
        .label {
            color: #888;
        }
        .value {
            font-weight: 600;
        }
        .progress-bar {
            background-color: #333;
            border-radius: 4px;
            height: 8px;
            margin: 8px 0;
            overflow: hidden;
        }
        .progress-fill {
            height: 100%;
            border-radius: 4px;
            transition: width 0.3s ease;
        }
        .badge {
            display: inline-block;
            padding: 2px 8px;
            border-radius: 4px;
            font-weight: bold;
            font-size: 0.85em;
        }
        .badge-low { background-color: #4CAF50; color: white; }
        .badge-medium { background-color: #FF9800; color: white; }
        .badge-high { background-color: #f44336; color: white; }
        .badge-critical { background-color: #B71C1C; color: white; }
        .green { color: #4CAF50; }
        .orange { color: #FF9800; }
        .red { color: #f44336; }
        .cache-type {
            background-color: #1a1a2e;
            border-radius: 4px;
            padding: 12px;
            margin-bottom: 8px;
        }
        .cache-type-title {
            font-weight: 600;
            color: #00d4ff;
            margin-bottom: 8px;
        }
        .actions {
            display: flex;
            gap: 8px;
            flex-wrap: wrap;
        }
        button {
            padding: 10px 20px;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.95em;
            font-weight: 500;
            transition: opacity 0.2s;
        }
        button:hover {
            opacity: 0.8;
        }
        .btn-danger {
            background-color: #f44336;
            color: white;
        }
        .btn-secondary {
            background-color: #333;
            color: white;
            border: 1px solid #555;
        }
        .btn-primary {
            background-color: #00d4ff;
            color: #1a1a2e;
        }
        .small {
            font-size: 0.85em;
            color: #888;
        }
        .divider {
            border-top: 1px solid #333;
            margin: 8px 0;
        }
        .time-saved {
            font-size: 1.1em;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <h1>Pezzottify Debug Dashboard</h1>

    <!-- Actions Section -->
    <div class="card">
        <h2>Actions</h2>
        <div class="actions">
            <form method="POST" action="/action/clear-cache" style="margin: 0;">
                <button type="submit" class="btn-danger">Clear Cache</button>
            </form>
            <form method="POST" action="/action/clear-statics-db" style="margin: 0;">
                <button type="submit" class="btn-danger">Clear Statics DB</button>
            </form>
            <form method="POST" action="/action/reset-metrics" style="margin: 0;">
                <button type="submit" class="btn-secondary">Reset Metrics</button>
            </form>
            <form method="POST" action="/action/test-whatsnew-notification" style="margin: 0;">
                <button type="submit" class="btn-primary">Test WhatsNew Notification</button>
            </form>
            <form method="POST" action="/action/force-token-refresh" style="margin: 0;">
                <button type="submit" class="btn-primary">Force Token Refresh</button>
            </form>
            <form method="GET" action="/" style="margin: 0;">
                <button type="submit" class="btn-primary">Refresh Dashboard</button>
            </form>
        </div>
    </div>
    
    <!-- Memory Info Section -->
    <div class="card">
        <h2>Memory Status</h2>
        <div class="row">
            <span class="label">Pressure Level:</span>
            <span class="${getBadgeClass(memoryInfo.pressureLevel)}">${memoryInfo.pressureLevel.name}</span>
        </div>
        <div class="row">
            <span class="label">Heap Usage:</span>
            <span class="value">${formatBytes(memoryInfo.usedBytes)} / ${formatBytes(memoryInfo.maxHeapBytes)}</span>
        </div>
        <div class="progress-bar">
            <div class="progress-fill" style="width: ${getUsagePercent(memoryInfo)}%; background-color: ${getProgressColor(memoryInfo)};"></div>
        </div>
        <div class="row">
            <span class="label">Available:</span>
            <span class="value">${formatBytes(memoryInfo.availableBytes)}</span>
        </div>
        <div class="row" style="margin-top: 12px;">
            <form method="POST" action="/action/refresh-memory" style="margin: 0;">
                <button type="submit" class="btn-secondary">Refresh Memory</button>
            </form>
        </div>
    </div>

    <!-- Cache Statistics Section -->
    <div class="card">
        <h2>Cache Statistics</h2>
        ${cacheMetrics.entries.joinToString("") { (type, metrics) -> buildCacheTypeHtml(type, metrics) }}
    </div>

    <!-- Performance Report Section -->
    <div class="card">
        <h2>Performance Metrics</h2>
        <div class="row">
            <span class="label">Estimated Time Saved:</span>
            <span class="value time-saved green">${formatNanos(performanceReport.estimatedTimeSavedNanos)}</span>
        </div>
        <div class="divider"></div>
        <div class="small" style="margin-bottom: 8px;">Latency Comparison (Cache vs DB):</div>
        ${buildLatencyComparisonHtml(performanceReport)}
    </div>

    <p class="small" style="margin-top: 20px; text-align: center;">
        Debug server running on port $DEFAULT_PORT
    </p>
</body>
</html>
        """.trimIndent()
    }

    private fun buildCacheTypeHtml(type: String, metrics: CacheMetrics): String {
        val hitRateClass = when {
            metrics.hitRate > 0.7 -> "green"
            metrics.hitRate > 0.4 -> "orange"
            else -> "red"
        }
        return """
        <div class="cache-type">
            <div class="cache-type-title">${type.replaceFirstChar { it.uppercase() }}</div>
            <div class="row">
                <span class="label">Entries:</span>
                <span class="value">${metrics.currentEntries}</span>
            </div>
            <div class="row">
                <span class="label">Size:</span>
                <span class="value">${formatBytes(metrics.currentSizeBytes)}</span>
            </div>
            <div class="row">
                <span class="label">Hits / Misses:</span>
                <span class="value">${metrics.hits} / ${metrics.misses}</span>
            </div>
            <div class="row">
                <span class="label">Hit Rate:</span>
                <span class="value $hitRateClass">${String.format("%.1f", metrics.hitRate * 100)}%</span>
            </div>
            <div class="row">
                <span class="label">Evictions / Expirations:</span>
                <span class="value small">${metrics.evictions} / ${metrics.expirations}</span>
            </div>
        </div>
        """
    }

    private fun buildLatencyComparisonHtml(report: CachePerformanceReport): String {
        val entries = report.avgCacheLatencyNanos.entries.filter { (type, cacheLatency) ->
            cacheLatency > 0 || (report.avgDbLatencyNanos[type] ?: 0.0) > 0
        }

        if (entries.isEmpty()) {
            return """<div class="small">No latency data available yet.</div>"""
        }

        return entries.joinToString("") { (type, cacheLatency) ->
            val dbLatency = report.avgDbLatencyNanos[type] ?: 0.0
            """
            <div class="row">
                <span class="label">${type.replaceFirstChar { it.uppercase() }}:</span>
                <span class="value small">Cache: ${formatNanos(cacheLatency.toLong())} | DB: ${formatNanos(dbLatency.toLong())}</span>
            </div>
            """
        }
    }

    private fun getBadgeClass(level: MemoryPressureLevel): String {
        return when (level) {
            MemoryPressureLevel.LOW -> "badge badge-low"
            MemoryPressureLevel.MEDIUM -> "badge badge-medium"
            MemoryPressureLevel.HIGH -> "badge badge-high"
            MemoryPressureLevel.CRITICAL -> "badge badge-critical"
        }
    }

    private fun getUsagePercent(memoryInfo: MemoryInfo): Int {
        return if (memoryInfo.maxHeapBytes > 0) {
            ((memoryInfo.usedBytes.toDouble() / memoryInfo.maxHeapBytes) * 100).toInt()
        } else 0
    }

    private fun getProgressColor(memoryInfo: MemoryInfo): String {
        val percent = getUsagePercent(memoryInfo) / 100.0
        return when {
            percent > 0.85 -> "#f44336"
            percent > 0.7 -> "#FF9800"
            else -> "#4CAF50"
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
}

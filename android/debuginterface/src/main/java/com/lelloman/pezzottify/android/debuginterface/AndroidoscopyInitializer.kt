package com.lelloman.pezzottify.android.debuginterface

import android.app.Application
import android.content.pm.ApplicationInfo
import android.util.Log
import com.lelloman.androidoscopy.ActionResult
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.dashboard.ButtonStyle
import com.lelloman.androidoscopy.buildinfo.BuildInfoDataProvider
import com.lelloman.androidoscopy.data.MemoryDataProvider
import com.lelloman.androidoscopy.data.NetworkDataProvider
import com.lelloman.androidoscopy.data.StorageDataProvider
import com.lelloman.androidoscopy.data.ThreadDataProvider
import com.lelloman.androidoscopy.permissions.PermissionsDataProvider
import com.lelloman.androidoscopy.prefs.SharedPreferencesDataProvider
import com.lelloman.androidoscopy.sqlite.SqliteDataProvider
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidoscopyInitializer @Inject constructor(
    @ApplicationContext private val context: android.content.Context,
    private val staticsCache: StaticsCache,
    private val tokenRefresher: TokenRefresher,
    private val diagnosticTools: PezzottifyDiagnosticTools,
) : AppInitializer {

    companion object {
        private const val TAG = "Androidoscopy"
    }

    override fun initialize() {
        try {
            val app = context.applicationContext as Application
            // Release exposes only explicit app tools, never the legacy database/token actions.
            if (app.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0) {
                Androidoscopy.init(app) {
                    appName = "Pezzottify"
                    enableLogging = false
                }
                diagnosticTools.register()
                return
            }
            val permissionsProvider = PermissionsDataProvider(app)
            val preferencesProvider = SharedPreferencesDataProvider(app)
            val sqliteProvider = SqliteDataProvider(app)

            Androidoscopy.init(app) {
                appName = "Pezzottify"
                enableAnrDetection()

                dashboard {
                    // Custom actions section
                    section("Actions") {
                        actions {
                            button(
                                label = "Force Token Refresh",
                                action = "force_token_refresh",
                                style = ButtonStyle.PRIMARY
                            )
                            button(
                                label = "Clear Cache",
                                action = "clear_cache",
                                style = ButtonStyle.SECONDARY
                            )
                            button(
                                label = "Force GC",
                                action = "force_gc",
                                style = ButtonStyle.SECONDARY
                            )
                        }
                    }

                    // System metrics
                    memorySection(includeActions = true)
                    storageSection()
                    networkSection()
                    threadSection()

                    // Main-thread stalls and their thread snapshots.
                    anrSection()

                    // SQLite databases
                    sqliteSection()

                    // SharedPreferences
                    sharedPreferencesSection()

                    // Permissions
                    permissionsSection()

                    // Build info
                    buildInfoSection()

                    // Logs
                    logsSection()
                }

                // Action handlers
                onAction("clear_cache") {
                    staticsCache.clearAll()
                    ActionResult.success("Statics cache cleared")
                }

                onAction("force_gc") {
                    System.gc()
                    ActionResult.success("GC requested")
                }

                onAction("force_token_refresh") {
                    val result = tokenRefresher.refreshTokens()
                    when (result) {
                        is TokenRefresher.RefreshResult.Success ->
                            ActionResult.success("Token refreshed successfully")
                        is TokenRefresher.RefreshResult.Failed ->
                            ActionResult.failure("Refresh failed: ${result.reason}")
                        TokenRefresher.RefreshResult.NotAvailable ->
                            ActionResult.failure("No refresh token available")
                        is TokenRefresher.RefreshResult.RateLimited ->
                            ActionResult.failure("Rate limited, retry after ${result.retryAfterMs}ms")
                    }
                }

                preferencesProvider.getActionHandlers().forEach { (name, handler) ->
                    onAction(name, handler)
                }
                sqliteProvider.getActionHandlers().forEach { (name, handler) ->
                    onAction(name, handler)
                }
                permissionsProvider.getActionHandlers().forEach { (name, handler) ->
                    onAction(name, handler)
                }
            }

            diagnosticTools.register()

            // Register built-in data providers
            Androidoscopy.registerDataProvider(MemoryDataProvider(app))
            Androidoscopy.registerDataProvider(StorageDataProvider(app))
            Androidoscopy.registerDataProvider(NetworkDataProvider(app))
            Androidoscopy.registerDataProvider(ThreadDataProvider())
            Androidoscopy.registerDataProvider(preferencesProvider)
            Androidoscopy.registerDataProvider(sqliteProvider)
            Androidoscopy.registerDataProvider(permissionsProvider)
            Androidoscopy.registerDataProvider(BuildInfoDataProvider(app))

            Log.i(TAG, "Androidoscopy initialized successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to initialize Androidoscopy", e)
        }
    }
}

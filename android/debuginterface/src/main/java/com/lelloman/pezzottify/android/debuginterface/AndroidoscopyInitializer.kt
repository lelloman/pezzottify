package com.lelloman.pezzottify.android.debuginterface

import android.app.Application
import android.util.Log
import com.lelloman.androidoscopy.ActionResult
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.dashboard.ButtonStyle
import com.lelloman.androidoscopy.data.MemoryDataProvider
import com.lelloman.androidoscopy.data.NetworkDataProvider
import com.lelloman.androidoscopy.data.StorageDataProvider
import com.lelloman.androidoscopy.data.ThreadDataProvider
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.runBlocking
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidoscopyInitializer @Inject constructor(
    @ApplicationContext private val context: android.content.Context,
    private val staticsCache: StaticsCache,
    private val tokenRefresher: TokenRefresher,
) : AppInitializer {

    companion object {
        private const val TAG = "Androidoscopy"
    }

    override fun initialize() {
        try {
            val app = context.applicationContext as Application

            Androidoscopy.init(app) {
                appName = "Pezzottify"

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
                    val result = runBlocking { tokenRefresher.refreshTokens() }
                    when (result) {
                        is TokenRefresher.RefreshResult.Success ->
                            ActionResult.success("Token refreshed successfully")
                        is TokenRefresher.RefreshResult.Failed ->
                            ActionResult.success("Refresh failed: ${result.reason}")
                        TokenRefresher.RefreshResult.NotAvailable ->
                            ActionResult.success("No refresh token available")
                        is TokenRefresher.RefreshResult.RateLimited ->
                            ActionResult.success("Rate limited, retry after ${result.retryAfterMs}ms")
                    }
                }
            }

            // Register built-in data providers
            Androidoscopy.registerDataProvider(MemoryDataProvider(app))
            Androidoscopy.registerDataProvider(StorageDataProvider(app))
            Androidoscopy.registerDataProvider(NetworkDataProvider(app))
            Androidoscopy.registerDataProvider(ThreadDataProvider())

            Log.i(TAG, "Androidoscopy initialized successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to initialize Androidoscopy", e)
        }
    }
}

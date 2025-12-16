package com.lelloman.pezzottify.android.debuginterface

import android.app.Application
import android.util.Log
import com.lelloman.androidoscopy.ActionResult
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.data.MemoryDataProvider
import com.lelloman.androidoscopy.data.NetworkDataProvider
import com.lelloman.androidoscopy.data.StorageDataProvider
import com.lelloman.androidoscopy.data.ThreadDataProvider
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidoscopyInitializer @Inject constructor(
    @ApplicationContext private val context: android.content.Context,
    private val staticsCache: StaticsCache
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

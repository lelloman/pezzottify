package com.lelloman.pezzottify.android.sync

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncManager
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent

class BackgroundSyncWorker(
    appContext: Context,
    workerParams: WorkerParameters,
) : CoroutineWorker(appContext, workerParams) {

    @EntryPoint
    @InstallIn(SingletonComponent::class)
    interface BackgroundSyncWorkerEntryPoint {
        fun authStore(): AuthStore
        fun syncManager(): SyncManager
        fun catalogSyncManager(): CatalogSyncManager
        fun loggerFactory(): LoggerFactory
    }

    private val entryPoint = EntryPointAccessors.fromApplication(
        appContext,
        BackgroundSyncWorkerEntryPoint::class.java,
    )
    private val authStore: AuthStore = entryPoint.authStore()
    private val syncManager: SyncManager = entryPoint.syncManager()
    private val catalogSyncManager: CatalogSyncManager = entryPoint.catalogSyncManager()
    private val logger: Logger by entryPoint.loggerFactory()

    override suspend fun doWork(): Result {
        val authState = authStore.getAuthState().value
        if (authState !is AuthState.LoggedIn) {
            logger.info("Background sync skipped: not logged in")
            return Result.success()
        }

        return try {
            logger.info("Background sync starting")
            syncManager.initialize()
            catalogSyncManager.catchUp()
            logger.info("Background sync completed")
            Result.success()
        } catch (e: Exception) {
            logger.error("Background sync failed", e)
            Result.retry()
        }
    }
}

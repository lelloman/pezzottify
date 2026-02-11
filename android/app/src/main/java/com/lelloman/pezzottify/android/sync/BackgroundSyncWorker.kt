package com.lelloman.pezzottify.android.sync

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncManager
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject

@HiltWorker
class BackgroundSyncWorker @AssistedInject constructor(
    @Assisted appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val authStore: AuthStore,
    private val syncManager: SyncManager,
    private val catalogSyncManager: CatalogSyncManager,
    loggerFactory: LoggerFactory,
) : CoroutineWorker(appContext, workerParams) {

    private val logger: Logger by loggerFactory

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

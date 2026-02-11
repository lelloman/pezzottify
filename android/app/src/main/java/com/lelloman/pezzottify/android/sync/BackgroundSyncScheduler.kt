package com.lelloman.pezzottify.android.sync

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.settings.BackgroundSyncInterval
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class BackgroundSyncScheduler @Inject constructor(
    private val authStore: AuthStore,
    private val userSettingsStore: UserSettingsStore,
    @ApplicationContext private val context: Context,
    loggerFactory: LoggerFactory,
) : AppInitializer {

    private val logger: Logger by loggerFactory
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    // Lazy to avoid circular dependency: WorkManager.getInstance() calls
    // Configuration.Provider before Hilt finishes injecting workerFactory.
    private val workManager by lazy { WorkManager.getInstance(context) }

    override fun initialize() {
        scope.launch {
            combine(
                authStore.getAuthState(),
                userSettingsStore.backgroundSyncInterval,
            ) { authState, interval ->
                authState to interval
            }.collect { (authState, interval) ->
                val isLoggedIn = authState is AuthState.LoggedIn
                logger.info("Background sync state changed: loggedIn=$isLoggedIn, interval=$interval")
                if (isLoggedIn && interval != BackgroundSyncInterval.Disabled) {
                    scheduleSync(interval)
                } else {
                    cancelSync()
                }
            }
        }
    }

    private fun scheduleSync(interval: BackgroundSyncInterval) {
        val nextRunTime = System.currentTimeMillis() + TimeUnit.MINUTES.toMillis(interval.minutes)
        val nextRunFormatted = dateFormat.format(Date(nextRunTime))
        logger.info(
            "Scheduling background sync: interval=${interval.minutes}min, next expected run ~$nextRunFormatted"
        )
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()

        val workRequest = PeriodicWorkRequestBuilder<BackgroundSyncWorker>(
            interval.minutes, TimeUnit.MINUTES
        )
            .setConstraints(constraints)
            .build()

        workManager.enqueueUniquePeriodicWork(
            WORK_NAME,
            ExistingPeriodicWorkPolicy.REPLACE,
            workRequest,
        )
    }

    private fun cancelSync() {
        logger.info("Cancelling background sync")
        workManager.cancelUniqueWork(WORK_NAME)
    }

    companion object {
        private const val WORK_NAME = "background_sync"
        private val dateFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US)
    }
}

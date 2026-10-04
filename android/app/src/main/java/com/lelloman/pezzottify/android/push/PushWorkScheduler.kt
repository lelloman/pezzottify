package com.lelloman.pezzottify.android.push

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import androidx.work.workDataOf
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.concurrent.TimeUnit
import javax.inject.Inject

/** Background work around push: server registration bookkeeping and wake-up syncs. */
interface PushWorkScheduler {
    fun scheduleServerRegistration()
    fun scheduleServerDeletion(endpoint: String)
    fun schedulePushSync()
}

class WorkManagerPushWorkScheduler @Inject constructor(
    @ApplicationContext private val context: Context,
) : PushWorkScheduler {

    private val workManager by lazy { WorkManager.getInstance(context) }
    private val networkConstraints = Constraints.Builder()
        .setRequiredNetworkType(NetworkType.CONNECTED)
        .build()

    override fun scheduleServerRegistration() {
        val request = OneTimeWorkRequestBuilder<PushServerSyncWorker>()
            .setInputData(workDataOf(PushServerSyncWorker.KEY_OPERATION to PushServerSyncWorker.OP_REGISTER))
            .setConstraints(networkConstraints)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        workManager.enqueueUniqueWork(WORK_REGISTER, ExistingWorkPolicy.REPLACE, request)
    }

    override fun scheduleServerDeletion(endpoint: String) {
        val request = OneTimeWorkRequestBuilder<PushServerSyncWorker>()
            .setInputData(
                workDataOf(
                    PushServerSyncWorker.KEY_OPERATION to PushServerSyncWorker.OP_DELETE,
                    PushServerSyncWorker.KEY_ENDPOINT to endpoint,
                )
            )
            .setConstraints(networkConstraints)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        workManager.enqueueUniqueWork(
            "$WORK_DELETE_PREFIX${endpoint.hashCode()}",
            ExistingWorkPolicy.KEEP,
            request,
        )
    }

    override fun schedulePushSync() {
        val request = OneTimeWorkRequestBuilder<PushSyncWorker>()
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .setConstraints(networkConstraints)
            .build()
        workManager.enqueueUniqueWork(WORK_SYNC, ExistingWorkPolicy.KEEP, request)
    }

    private companion object {
        const val WORK_REGISTER = "push_server_register"
        const val WORK_DELETE_PREFIX = "push_server_delete_"
        const val WORK_SYNC = "push_sync"
    }
}

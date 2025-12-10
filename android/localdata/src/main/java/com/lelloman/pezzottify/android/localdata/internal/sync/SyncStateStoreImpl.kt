package com.lelloman.pezzottify.android.localdata.internal.sync

import android.content.Context
import com.lelloman.pezzottify.android.domain.sync.SyncStateStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext

/**
 * Implementation of [SyncStateStore] using SharedPreferences.
 *
 * Persists the sync cursor (sequence number) to enable efficient catch-up sync
 * when the app restarts or reconnects.
 */
internal class SyncStateStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : SyncStateStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    private val mutableCursor by lazy {
        MutableStateFlow(prefs.getLong(KEY_CURSOR, DEFAULT_CURSOR))
    }

    private val mutableNeedsFullSync by lazy {
        MutableStateFlow(prefs.getBoolean(KEY_NEEDS_FULL_SYNC, DEFAULT_NEEDS_FULL_SYNC))
    }

    override val cursor: StateFlow<Long> = mutableCursor.asStateFlow()

    override fun getCurrentCursor(): Long = mutableCursor.value

    override suspend fun saveCursor(cursor: Long) {
        withContext(dispatcher) {
            mutableCursor.value = cursor
            prefs.edit().putLong(KEY_CURSOR, cursor).commit()
        }
    }

    override suspend fun clearCursor() {
        withContext(dispatcher) {
            mutableCursor.value = DEFAULT_CURSOR
            mutableNeedsFullSync.value = DEFAULT_NEEDS_FULL_SYNC
            prefs.edit()
                .remove(KEY_CURSOR)
                .remove(KEY_NEEDS_FULL_SYNC)
                .commit()
        }
    }

    override fun needsFullSync(): Boolean = mutableNeedsFullSync.value

    override suspend fun setNeedsFullSync(needsFullSync: Boolean) {
        withContext(dispatcher) {
            mutableNeedsFullSync.value = needsFullSync
            prefs.edit().putBoolean(KEY_NEEDS_FULL_SYNC, needsFullSync).commit()
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "SyncStateStore"
        private const val KEY_CURSOR = "sync_cursor"
        private const val KEY_NEEDS_FULL_SYNC = "needs_full_sync"
        private const val DEFAULT_CURSOR = 0L
        private const val DEFAULT_NEEDS_FULL_SYNC = true // Default to needing full sync
    }
}

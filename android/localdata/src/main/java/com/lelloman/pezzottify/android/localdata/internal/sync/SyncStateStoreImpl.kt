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
            prefs.edit().remove(KEY_CURSOR).commit()
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "SyncStateStore"
        private const val KEY_CURSOR = "sync_cursor"
        private const val DEFAULT_CURSOR = 0L
    }
}

package com.lelloman.pezzottify.android.localdata.internal.catalogsync

import android.content.Context
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext

/**
 * Implementation of [CatalogSyncStore] using SharedPreferences.
 *
 * Persists the catalog sync cursor (sequence number) to enable efficient
 * catch-up when the app reconnects after being offline.
 */
internal class CatalogSyncStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : CatalogSyncStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    private val mutableCurrentSeq by lazy {
        MutableStateFlow(prefs.getLong(KEY_CURRENT_SEQ, DEFAULT_SEQ))
    }

    override val currentSeq: StateFlow<Long> = mutableCurrentSeq.asStateFlow()

    override suspend fun setCurrentSeq(seq: Long) {
        withContext(dispatcher) {
            mutableCurrentSeq.value = seq
            prefs.edit().putLong(KEY_CURRENT_SEQ, seq).commit()
        }
    }

    override suspend fun clear() {
        withContext(dispatcher) {
            mutableCurrentSeq.value = DEFAULT_SEQ
            prefs.edit().remove(KEY_CURRENT_SEQ).commit()
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "CatalogSyncStore"
        private const val KEY_CURRENT_SEQ = "catalog_sync_seq"
        private const val DEFAULT_SEQ = 0L
    }
}

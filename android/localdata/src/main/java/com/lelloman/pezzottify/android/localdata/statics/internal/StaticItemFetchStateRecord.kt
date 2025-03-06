package com.lelloman.pezzottify.android.localdata.statics.internal

internal data class StaticItemFetchStateRecord(
    val itemId: String,
    val loading: Boolean,
    val errorReason: String?,
    val lastUpdated: Long,
    val created: Long,
)
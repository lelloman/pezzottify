package com.lelloman.pezzottify.android.localdata.config

import kotlinx.coroutines.flow.StateFlow

interface ConfigStore {

    val baseUrl: StateFlow<String>

    suspend fun setBaseUrl(baseUrl: String)
}
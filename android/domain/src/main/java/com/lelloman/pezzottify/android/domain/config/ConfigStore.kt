package com.lelloman.pezzottify.android.domain.config

import kotlinx.coroutines.flow.StateFlow

interface ConfigStore {

    val baseUrl: StateFlow<String>

    suspend fun setBaseUrl(baseUrl: String): SetBaseUrlResult

    sealed interface SetBaseUrlResult {
        data object Success : SetBaseUrlResult
        data object InvalidUrl : SetBaseUrlResult
    }
}
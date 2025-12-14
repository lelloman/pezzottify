package com.lelloman.pezzottify.android.remoteapi.internal

import okhttp3.OkHttpClient
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Factory for creating OkHttpClient.Builder instances.
 */
@Singleton
open class OkHttpClientFactory @Inject constructor() {
    /**
     * Creates a new OkHttpClient.Builder.
     *
     * @param baseUrl The base URL (unused, kept for API compatibility)
     * @return OkHttpClient.Builder
     */
    open fun createBuilder(baseUrl: String): OkHttpClient.Builder {
        return OkHttpClient.Builder()
    }
}

package com.lelloman.pezzottify.android.domain.auth

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import kotlinx.coroutines.flow.StateFlow

interface AuthStore : AppInitializer {

    fun getAuthState(): StateFlow<AuthState>

    suspend fun storeAuthState(newAuthState: AuthState): Result<Unit>

    fun getLastUsedHandle(): String?

    fun getLastUsedBaseUrl(): String?

    suspend fun storeLastUsedCredentials(handle: String, baseUrl: String)
}
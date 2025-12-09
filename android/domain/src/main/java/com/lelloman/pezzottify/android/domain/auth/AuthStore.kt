package com.lelloman.pezzottify.android.domain.auth

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import kotlinx.coroutines.flow.StateFlow

interface AuthStore : AppInitializer {

    fun getAuthState(): StateFlow<AuthState>

    suspend fun storeAuthState(newAuthState: AuthState): Result<Unit>

    fun getLastUsedHandle(): String?

    suspend fun storeLastUsedHandle(handle: String)
}
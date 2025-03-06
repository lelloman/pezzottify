package com.lelloman.pezzottify.android.localdata.auth

import kotlinx.coroutines.flow.StateFlow

interface AuthStore {

    fun getAuthState(): StateFlow<AuthState>

    suspend fun storeAuthState(newAuthState: AuthState): Result<Unit>
}
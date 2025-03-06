package com.lelloman.pezzottify.android.localdata.auth

import kotlinx.coroutines.flow.Flow

interface AuthStore {

    fun getAuthState(): Flow<AuthState>

    suspend fun storeAuthState(newAuthState: AuthState): Result<Void>
}
package com.lelloman.pezzottify.android.app.localdata

import com.lelloman.pezzottify.android.app.domain.LoginOperation
import com.lelloman.pezzottify.android.app.domain.LoginState
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext

class StaticFetcherOnLogin(private val dispatcher: CoroutineDispatcher) : LoginOperation {
    override suspend fun invoke(loginState: LoginState.LoggedIn): Boolean =
        withContext(dispatcher) {
            true
        }
}
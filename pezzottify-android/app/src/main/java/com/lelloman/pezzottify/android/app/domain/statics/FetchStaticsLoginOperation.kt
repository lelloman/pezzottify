package com.lelloman.pezzottify.android.app.domain.statics

import com.lelloman.pezzottify.android.app.domain.login.LoginOperation
import com.lelloman.pezzottify.android.app.domain.login.LoginState
import javax.inject.Inject

class FetchStaticsLoginOperation @Inject constructor(private val staticsStore: StaticsStore) :
    LoginOperation {

    override suspend fun invoke(loginState: LoginState.LoggedIn) = staticsStore.fetchStatics()
}
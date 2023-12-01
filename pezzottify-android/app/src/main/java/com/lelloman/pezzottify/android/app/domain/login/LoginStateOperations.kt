package com.lelloman.pezzottify.android.app.domain.login

fun interface LoginOperation {
    suspend operator fun invoke(loginState: LoginState.LoggedIn): Boolean
}

fun interface LogoutOperation {
    suspend operator fun invoke()
}

class LoginStateOperationsCollector(
    private val loginOperations: Set<LoginOperation>,
    private val logoutOperation: Set<LogoutOperation>
) {

    fun register(loginManager: LoginManager) =
        loginManager.registerOperations(loginOperations, logoutOperation)
}
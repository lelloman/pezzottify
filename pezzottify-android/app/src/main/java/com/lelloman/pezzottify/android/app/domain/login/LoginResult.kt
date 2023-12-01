package com.lelloman.pezzottify.android.app.domain.login

sealed class LoginResult {

    sealed class Failure : LoginResult() {
        object Network : Failure()
        object Credentials : Failure()
        object Unknown : Failure()
    }

    data class Success(val authToken: String) : LoginResult()
}
package com.lelloman.pezzottify.remoteapi

sealed class LoginResponse {

    data class Success(val authToken: String) : LoginResponse()

    object InvalidCredentials : LoginResponse()
}
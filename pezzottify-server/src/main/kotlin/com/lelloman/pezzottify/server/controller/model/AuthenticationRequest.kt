package com.lelloman.pezzottify.server.controller.model

data class AuthenticationRequest(
    val username: String = "",
    val password: String = "",
)
package com.lelloman.pezzottify.android.domain.auth

fun bearerAuthorization(token: String): String {
    require(token.isNotBlank()) { "Authorization token must not be blank" }
    return "Bearer $token"
}

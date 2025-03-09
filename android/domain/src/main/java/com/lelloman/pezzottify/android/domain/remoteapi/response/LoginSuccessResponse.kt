package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.Serializable

@Serializable
data class LoginSuccessResponse (
    val token: String,
)
package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.Serializable

@Serializable
internal data class LoginRequest(
    val userHandle: String,
    val password: String,
)
package com.lelloman.pezzottify.android.domain.auth

fun interface AssistantSessionCleaner {
    suspend fun clearSession()
}

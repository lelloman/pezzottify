package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthStore
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

class InitializeApp @Inject constructor(private val authStore: AuthStore) : UseCase() {
    operator fun invoke() {
        runBlocking { authStore.initialize() }
    }
}
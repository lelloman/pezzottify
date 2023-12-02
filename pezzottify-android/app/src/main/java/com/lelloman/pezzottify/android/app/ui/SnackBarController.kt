package com.lelloman.pezzottify.android.app.ui

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SnackBarController @Inject constructor() {

    private val mutableSnacks = MutableSharedFlow<Snack?>()
    val snacks = mutableSnacks.asSharedFlow()

    suspend fun showSnack(message: String) {
        showSnack(Snack(message))
    }

    suspend fun showSnack(snack: Snack) {
        mutableSnacks.emit(snack)
    }

    data class Snack(
        val message: String,
    )
}
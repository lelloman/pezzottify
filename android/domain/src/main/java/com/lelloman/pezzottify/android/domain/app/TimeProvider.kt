package com.lelloman.pezzottify.android.domain.app

fun interface TimeProvider {
    fun nowUtcMs(): Long
}
package com.lelloman.pezzottify.android.logger

interface Logger {
    fun debug(message: String, throwable: Throwable? = null)

    fun info(message: String, throwable: Throwable? = null)

    fun warn(message: String, throwable: Throwable? = null)

    fun error(message: String, throwable: Throwable? = null)
}
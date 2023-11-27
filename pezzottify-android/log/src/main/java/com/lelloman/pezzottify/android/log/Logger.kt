package com.lelloman.pezzottify.android.log

interface Logger {
    fun trace(msg: String, throwable: Throwable? = null)
    fun debug(msg: String, throwable: Throwable? = null)
    fun info(msg: String, throwable: Throwable? = null)
    fun warn(msg: String, throwable: Throwable? = null)
    fun error(msg: String, throwable: Throwable? = null)
}
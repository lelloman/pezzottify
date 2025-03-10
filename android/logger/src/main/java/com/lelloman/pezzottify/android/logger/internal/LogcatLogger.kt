package com.lelloman.pezzottify.android.logger.internal

import android.util.Log
import com.lelloman.pezzottify.android.logger.Logger

internal class LogcatLogger(
    private val tag: String,
) : Logger {
    override fun debug(message: String, throwable: Throwable?) {
        Log.d(tag, message, throwable)
    }

    override fun info(message: String, throwable: Throwable?) {
        Log.i(tag, message, throwable)
    }

    override fun warn(message: String, throwable: Throwable?) {
        Log.w(tag, message, throwable)
    }

    override fun error(message: String, throwable: Throwable?) {
        Log.e(tag, message, throwable)
    }
}
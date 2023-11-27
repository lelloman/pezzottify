package com.lelloman.pezzottify.android.log

import android.util.Log

internal class LogcatLogger(private val tag: String) : Logger {

    override fun trace(msg: String, throwable: Throwable?) = log(Log::v, msg, throwable)

    override fun debug(msg: String, throwable: Throwable?) = log(Log::d, msg, throwable)

    override fun info(msg: String, throwable: Throwable?) = log(Log::i, msg, throwable)

    override fun warn(msg: String, throwable: Throwable?) = log(Log::w, msg, throwable)

    override fun error(msg: String, throwable: Throwable?) = log(Log::e, msg, throwable)

    private fun log(func: (String, String, Throwable?) -> Int, msg: String, throwable: Throwable?) {
        func(tag, msg, throwable)
    }
}
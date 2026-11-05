package com.lelloman.pezzottify.android.localdata.internal.security

import com.lelloman.pezzottify.android.logger.Logger

internal class WarningLogger : Logger {
    val warnings = mutableListOf<String>()
    override fun debug(message: String, throwable: Throwable?) {}
    override fun info(message: String, throwable: Throwable?) {}
    override fun warn(message: String, throwable: Throwable?) { warnings += message }
    override fun error(message: String, throwable: Throwable?) {}
}

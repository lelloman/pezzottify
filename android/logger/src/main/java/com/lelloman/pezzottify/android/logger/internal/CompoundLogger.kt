package com.lelloman.pezzottify.android.logger.internal

import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.flow.StateFlow

internal class PezzottifyLogger(
    private val loggers: List<Logger>,
    private val logLevelProvider: StateFlow<LogLevel>,
) : Logger {
    override fun debug(message: String, throwable: Throwable?) {
        if (logLevelProvider.value.ordinal <= LogLevel.Debug.ordinal) {
            loggers.forEach {
                it.debug(message, throwable)
            }
        }
    }

    override fun info(message: String, throwable: Throwable?) {
        if (logLevelProvider.value.ordinal <= LogLevel.Info.ordinal) {
            loggers.forEach {
                it.info(message, throwable)
            }
        }
    }

    override fun warn(message: String, throwable: Throwable?) {
        if (logLevelProvider.value.ordinal <= LogLevel.Warn.ordinal) {
            loggers.forEach {
                it.warn(message, throwable)
            }
        }
    }

    override fun error(message: String, throwable: Throwable?) {
        if (logLevelProvider.value.ordinal <= LogLevel.Error.ordinal) {
            loggers.forEach {
                it.error(message, throwable)
            }
        }
    }
}
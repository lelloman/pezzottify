package com.lelloman.pezzottify.android.logger

import com.lelloman.pezzottify.android.logger.internal.LogcatLogger
import com.lelloman.pezzottify.android.logger.internal.PezzottifyLogger
import kotlinx.coroutines.flow.StateFlow
import kotlin.reflect.KClass

class LoggerFactory(private val logLevelProvider: StateFlow<LogLevel>) {

    fun getLogger(clazz: KClass<*>): Logger {
        val tag = clazz.simpleName ?: "Unknown"
        return getLogger(tag)
    }

    fun getLogger(customTag: String): Logger {
        return PezzottifyLogger(listOf(LogcatLogger(customTag)), logLevelProvider)
    }
}
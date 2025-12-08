package com.lelloman.pezzottify.android.logger

import com.lelloman.pezzottify.android.logger.internal.ConditionalFileLogger
import com.lelloman.pezzottify.android.logger.internal.FileLogger
import com.lelloman.pezzottify.android.logger.internal.LogcatLogger
import com.lelloman.pezzottify.android.logger.internal.PezzottifyLogger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import java.io.File
import kotlin.reflect.KClass
import kotlin.reflect.KProperty

class LoggerFactory(
    private val logLevelProvider: StateFlow<LogLevel>,
    private val fileLoggingEnabled: StateFlow<Boolean> = MutableStateFlow(false),
    private val logDir: File? = null,
) {

    fun getLogger(clazz: KClass<*>): Logger {
        val tag = clazz.simpleName ?: "Unknown"
        return getLogger(tag)
    }

    fun getLogger(customTag: String): Logger {
        val loggers = mutableListOf<Logger>(LogcatLogger(customTag))
        if (logDir != null) {
            loggers.add(
                ConditionalFileLogger(
                    delegate = FileLogger(customTag, logDir),
                    enabledProvider = fileLoggingEnabled,
                )
            )
        }
        return PezzottifyLogger(loggers, logLevelProvider)
    }

    /**
     * Allows a user that has the LoggerFactory injected in the constructor to declare
     * a Logger property by delegation:
     *
     * class Foo(loggerFactory: LoggerFactory) {
     *
     *  private val logger by loggerFactory
     */
    operator fun getValue(thisRef: Any, property: KProperty<*>): Logger {
        return this.getLogger(thisRef::class)
    }
}



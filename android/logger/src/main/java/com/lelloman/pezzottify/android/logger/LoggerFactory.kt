package com.lelloman.pezzottify.android.logger

import com.lelloman.pezzottify.android.logger.internal.LogcatLogger
import com.lelloman.pezzottify.android.logger.internal.PezzottifyLogger
import kotlinx.coroutines.flow.StateFlow
import kotlin.reflect.KClass
import kotlin.reflect.KProperty

class LoggerFactory(private val logLevelProvider: StateFlow<LogLevel>) {

    fun getLogger(clazz: KClass<*>): Logger {
        val tag = clazz.simpleName ?: "Unknown"
        return getLogger(tag)
    }

    fun getLogger(customTag: String): Logger {
        return PezzottifyLogger(listOf(LogcatLogger(customTag)), logLevelProvider)
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



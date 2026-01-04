package com.lelloman.pezzottify.android.assistant

import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.simpleaiassistant.util.AssistantLogger

/**
 * Adapter that bridges the Pezzottify logger to the simple-ai-assistant logger interface.
 */
class PezzottifyAssistantLogger(
    private val loggerFactory: LoggerFactory
) : AssistantLogger {

    private val loggers = mutableMapOf<String, com.lelloman.pezzottify.android.logger.Logger>()

    private fun getLogger(tag: String): com.lelloman.pezzottify.android.logger.Logger {
        return loggers.getOrPut(tag) { loggerFactory.getLogger(tag) }
    }

    override fun debug(tag: String, message: String) {
        getLogger(tag).debug(message)
    }

    override fun info(tag: String, message: String) {
        getLogger(tag).info(message)
    }

    override fun warn(tag: String, message: String) {
        getLogger(tag).warn(message)
    }

    override fun error(tag: String, message: String, throwable: Throwable?) {
        getLogger(tag).error(message, throwable)
    }
}

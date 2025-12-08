package com.lelloman.pezzottify.android.logger.internal

import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.flow.StateFlow

/**
 * A Logger wrapper that conditionally delegates to the underlying logger
 * based on an enabled state provided via StateFlow.
 *
 * This allows file logging to be dynamically enabled/disabled at runtime
 * without recreating logger instances.
 */
internal class ConditionalFileLogger(
    private val delegate: Logger,
    private val enabledProvider: StateFlow<Boolean>,
) : Logger {

    override fun debug(message: String, throwable: Throwable?) {
        // Debug is already a no-op in FileLogger, skip the enabled check
    }

    override fun info(message: String, throwable: Throwable?) {
        if (enabledProvider.value) {
            delegate.info(message, throwable)
        }
    }

    override fun warn(message: String, throwable: Throwable?) {
        if (enabledProvider.value) {
            delegate.warn(message, throwable)
        }
    }

    override fun error(message: String, throwable: Throwable?) {
        if (enabledProvider.value) {
            delegate.error(message, throwable)
        }
    }
}

package com.lelloman.simpleaiassistant.util

/**
 * Simple logging interface for the AI assistant module.
 * The host application can provide an implementation to integrate with their logging system.
 */
interface AssistantLogger {
    fun debug(tag: String, message: String)
    fun info(tag: String, message: String)
    fun warn(tag: String, message: String)
    fun error(tag: String, message: String, throwable: Throwable? = null)
}

/**
 * No-op logger implementation (default).
 */
object NoOpAssistantLogger : AssistantLogger {
    override fun debug(tag: String, message: String) {}
    override fun info(tag: String, message: String) {}
    override fun warn(tag: String, message: String) {}
    override fun error(tag: String, message: String, throwable: Throwable?) {}
}

/**
 * Simple Android Logcat logger.
 */
object LogcatAssistantLogger : AssistantLogger {
    override fun debug(tag: String, message: String) {
        android.util.Log.d(tag, message)
    }

    override fun info(tag: String, message: String) {
        android.util.Log.i(tag, message)
    }

    override fun warn(tag: String, message: String) {
        android.util.Log.w(tag, message)
    }

    override fun error(tag: String, message: String, throwable: Throwable?) {
        if (throwable != null) {
            android.util.Log.e(tag, message, throwable)
        } else {
            android.util.Log.e(tag, message)
        }
    }
}

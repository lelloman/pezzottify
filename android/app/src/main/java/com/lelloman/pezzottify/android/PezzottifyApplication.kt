package com.lelloman.pezzottify.android

import android.app.Application
import com.lelloman.pezzottify.android.domain.usecase.InitializeApp
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApplication : Application() {

    @Inject
    lateinit var initializeApp: InitializeApp

    @Inject
    lateinit var loggerFactory: LoggerFactory

    override fun onCreate() {
        super.onCreate()
        setupUncaughtExceptionHandler()
        initializeApp()
    }

    private fun setupUncaughtExceptionHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        val logger = loggerFactory.getLogger("UncaughtException")

        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                logger.error("Uncaught exception in thread '${thread.name}'", throwable)
            } catch (_: Exception) {
                // Ignore any logging failures - we don't want to cause another crash
            }
            // Delegate to default handler so the app crashes normally
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }
}
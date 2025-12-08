package com.lelloman.pezzottify.android.logger.internal

import com.lelloman.pezzottify.android.logger.Logger
import java.io.File
import java.io.FileWriter
import java.io.PrintWriter
import java.io.StringWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Logger implementation that writes logs to rolling files.
 * Only captures Info, Warn, and Error level logs (Debug is a no-op).
 *
 * Files are named pezzottify_0.log through pezzottify_4.log.
 * File 0 is always the current/active file.
 * When file 0 exceeds maxFileSize, files are rotated (oldest deleted, others renamed).
 */
internal class FileLogger(
    private val tag: String,
    private val logDir: File,
    private val maxFileSize: Long = 1 * 1024 * 1024L, // 1MB
    private val maxFiles: Int = 5,
) : Logger {

    private val dateFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)

    init {
        logDir.mkdirs()
    }

    override fun debug(message: String, throwable: Throwable?) {
        // NO-OP: File logger only captures Info and above
    }

    override fun info(message: String, throwable: Throwable?) {
        writeLog("INFO", message, throwable)
    }

    override fun warn(message: String, throwable: Throwable?) {
        writeLog("WARN", message, throwable)
    }

    override fun error(message: String, throwable: Throwable?) {
        writeLog("ERROR", message, throwable)
    }

    @Synchronized
    private fun writeLog(level: String, message: String, throwable: Throwable?) {
        try {
            val currentFile = getCurrentLogFile()

            // Check if rotation is needed
            if (currentFile.exists() && currentFile.length() >= maxFileSize) {
                rotateFiles()
            }

            val timestamp = dateFormat.format(Date())
            val logEntry = buildString {
                append("[$timestamp] [$level] [$tag] $message")
                if (throwable != null) {
                    append("\n")
                    append(getStackTraceString(throwable))
                }
                append("\n")
            }

            FileWriter(currentFile, true).use { writer ->
                writer.write(logEntry)
            }
        } catch (e: Exception) {
            // Silently fail - we don't want logging failures to crash the app
        }
    }

    private fun getCurrentLogFile(): File {
        return File(logDir, "pezzottify_0.log")
    }

    private fun rotateFiles() {
        // Delete the oldest file if it exists
        val oldestFile = File(logDir, "pezzottify_${maxFiles - 1}.log")
        if (oldestFile.exists()) {
            oldestFile.delete()
        }

        // Rename files: 3 -> 4, 2 -> 3, 1 -> 2, 0 -> 1
        for (i in (maxFiles - 2) downTo 0) {
            val sourceFile = File(logDir, "pezzottify_$i.log")
            val targetFile = File(logDir, "pezzottify_${i + 1}.log")
            if (sourceFile.exists()) {
                sourceFile.renameTo(targetFile)
            }
        }
    }

    private fun getStackTraceString(throwable: Throwable): String {
        val sw = StringWriter()
        val pw = PrintWriter(sw)
        throwable.printStackTrace(pw)
        pw.flush()
        return sw.toString()
    }
}

package com.lelloman.pezzottify.android.logging

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * Manages log files for the file-based logging feature.
 * Handles log directory access, file enumeration, cleanup, and sharing.
 */
class LogFileManager(private val context: Context) {

    /**
     * Directory where log files are stored.
     */
    val logDir: File = File(context.filesDir, "logs").also { it.mkdirs() }

    /**
     * Returns all log files sorted by name (oldest first).
     */
    fun getLogFiles(): List<File> = logDir.listFiles()
        ?.filter { it.name.startsWith("pezzottify_") && it.name.endsWith(".log") }
        ?.sortedBy { it.name }
        ?: emptyList()

    /**
     * Returns the concatenated content of all log files.
     * Files are read in order (oldest first) and joined together.
     */
    fun getLogContent(): String = getLogFiles()
        .joinToString(separator = "") { file ->
            file.readText()
        }

    /**
     * Returns true if there are any log files.
     */
    fun hasLogs(): Boolean = getLogFiles().isNotEmpty()

    /**
     * Returns the total size of all log files in bytes.
     */
    fun getTotalLogSize(): Long = getLogFiles().sumOf { it.length() }

    /**
     * Returns a human-readable string of the total log size.
     */
    fun getFormattedLogSize(): String {
        val bytes = getTotalLogSize()
        return when {
            bytes < 1024 -> "$bytes B"
            bytes < 1024 * 1024 -> "%.1f KB".format(bytes / 1024.0)
            else -> "%.1f MB".format(bytes / (1024.0 * 1024.0))
        }
    }

    /**
     * Deletes all log files.
     */
    fun clearLogs() {
        getLogFiles().forEach { it.delete() }
    }

    /**
     * Creates an Intent to share all log files as a zip archive.
     * Uses FileProvider for secure file sharing.
     */
    fun createShareIntent(): Intent {
        val zipFile = createZipFile()
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            zipFile
        )
        return Intent.createChooser(
            Intent(Intent.ACTION_SEND).apply {
                type = "application/zip"
                putExtra(Intent.EXTRA_STREAM, uri)
                putExtra(Intent.EXTRA_SUBJECT, "Pezzottify Logs")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            },
            "Share Pezzottify Logs"
        )
    }

    /**
     * Creates a zip file containing all log files.
     * The zip is stored in the cache directory and can be shared via FileProvider.
     */
    private fun createZipFile(): File {
        val zipFile = File(context.cacheDir, "pezzottify_logs.zip")
        // Delete existing zip if present
        if (zipFile.exists()) {
            zipFile.delete()
        }

        ZipOutputStream(FileOutputStream(zipFile)).use { zos ->
            getLogFiles().forEach { logFile ->
                zos.putNextEntry(ZipEntry(logFile.name))
                logFile.inputStream().use { input ->
                    input.copyTo(zos)
                }
                zos.closeEntry()
            }
        }
        return zipFile
    }
}

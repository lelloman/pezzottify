package com.lelloman.pezzottify.android.localdata.internal.statics

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class StaticsDbSizeCalculator @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    fun getDatabaseSizeBytes(): Long {
        val dbPath = context.getDatabasePath(StaticsDb.NAME)
        val walPath = File(dbPath.absolutePath + "-wal")
        val shmPath = File(dbPath.absolutePath + "-shm")

        return listOf(dbPath, walPath, shmPath)
            .filter { it.exists() }
            .sumOf { it.length() }
    }
}

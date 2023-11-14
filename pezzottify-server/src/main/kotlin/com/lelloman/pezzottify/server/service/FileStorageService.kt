package com.lelloman.pezzottify.server.service

import org.springframework.context.annotation.Profile
import org.springframework.stereotype.Service
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileNotFoundException
import java.io.InputStream
import java.io.OutputStream
import java.util.UUID

@Service
interface FileStorageService {

    val totalSize: Long

    fun create(inputStream: InputStream): Creation

    fun open(id: String): InputStream

    fun remove(id: String)

    data class Creation(val id: String, val size: Long)
}

//@Service
//@Profile("dev")
//class LocalFsFileStorageService(
//    private val rootDir: File,
//) : FileStorageService {
//
//    override fun create(inputStream: InputStream): FileStorageService.Creation {
//        TODO("Not yet implemented")
//    }
//
//    override fun open(id: String): InputStream {
//        TODO("Not yet implemented")
//    }
//}

@Service
@Profile("test", "dev")
class InMemoryFileStorageService : FileStorageService {

    private val files = mutableMapOf<String, ByteArray>()

    override val totalSize get() = files.values.sumOf { it.size.toLong() }

    override fun create(inputStream: InputStream): FileStorageService.Creation {
        var id: String
        do {
            id = UUID.randomUUID().toString()
        } while (files.containsKey(id))
        val bytes = inputStream.readAllBytes()
        files[id] = bytes
        return FileStorageService.Creation(id, bytes.size.toLong())
    }

    override fun open(id: String): InputStream {
        return files[id]?.inputStream() ?: throw FileNotFoundException()
    }

    override fun remove(id: String) {
        files.remove(id)
    }
}
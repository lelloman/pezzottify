package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.AudioTrackRepository
import com.lelloman.pezzottify.server.model.AudioTrack
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Service
import org.springframework.web.multipart.MultipartFile
import java.io.File
import java.io.FileInputStream

@Service
class AudioTrackUploader(
    @Autowired private val audioTrackRepository: AudioTrackRepository,
    @Autowired private val storageService: FileStorageService,
    @Autowired private val audioTrackDecoder: AudioTrackDecoder,
) {

    fun newOperation() = object : UploadOperation {
        private val pendingTracks = mutableListOf<AudioTrack>()
        private var tmpFile: File? = null

        override fun createAudioTrack(multipartFile: MultipartFile, name: String): AudioTrack {
            tmpFile = File.createTempFile("upload", "tmp")
            multipartFile.inputStream.copyTo(tmpFile!!.outputStream())
            val decoded = audioTrackDecoder.decode(tmpFile!!)
                ?: throw DecodeAudioTrackException("Could not decode audio track \"$name\"")
            val creation = storageService.create(FileInputStream(tmpFile!!))

            val audioTrack = AudioTrack(
                id = creation.id,
                size = creation.size,
                name = name,
                durationMs = decoded.durationMs,
                bitRate = decoded.bitRate,
                sampleRate = decoded.sampleRate,
                type = decoded.type,
            )
            val createdTrack = audioTrackRepository.save(audioTrack)
            pendingTracks.add(createdTrack)
            tmpFile!!.delete()
            tmpFile = null
            return createdTrack
        }

        override fun aborted() {
            tmpFile?.delete()
            audioTrackRepository.deleteAll(pendingTracks)
        }

        override fun succeeded() {
            tmpFile?.delete()
            audioTrackRepository.saveAll(pendingTracks.map { it.copy(orphan = false) })
        }
    }

    interface UploadOperation {
        fun createAudioTrack(multipartFile: MultipartFile, name: String): AudioTrack
        fun aborted()
        fun succeeded()
    }

    class DecodeAudioTrackException(message: String) : Throwable(message)
}
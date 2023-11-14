package com.lelloman.pezzottify.server.service

import com.mpatric.mp3agic.Mp3File
import org.springframework.stereotype.Component
import java.io.File

@Component
class AudioTrackDecoder {

    fun decode(file: File) = try {
        val mp3File = Mp3File(file)

        AudioTrackSpec(
            durationMs = mp3File.lengthInMilliseconds,
            bitRate = mp3File.bitrate,
            sampleRate = mp3File.sampleRate,
        )
    } catch (e: Throwable) {
        null
    }

    data class AudioTrackSpec(
        val durationMs: Long,
        val bitRate: Int,
        val sampleRate: Int,
    )
}
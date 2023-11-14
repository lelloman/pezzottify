package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.model.AudioTrack
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
            type = AudioTrack.Type.MP3,
        )
    } catch (e: Throwable) {
        null
    }

    data class AudioTrackSpec(
        val durationMs: Long,
        val bitRate: Int,
        val sampleRate: Int,
        val type: AudioTrack.Type,
    )
}
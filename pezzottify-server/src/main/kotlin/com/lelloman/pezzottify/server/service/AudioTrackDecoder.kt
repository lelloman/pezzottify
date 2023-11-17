package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.model.AudioTrack
import org.jaudiotagger.audio.AudioFile
import org.jaudiotagger.audio.AudioFileIO
import org.springframework.stereotype.Component
import java.io.File
import kotlin.math.roundToLong

@Component
class AudioTrackDecoder {

    fun decode(file: File): AudioTrackSpec? {
        val audioFile = readAudioFile(file) ?: return null
        val type = when (audioFile.ext.lowercase()) {
            "mp3" -> AudioTrack.Type.MP3
            "flac" -> AudioTrack.Type.FLAC
            else -> return null
        }
        val header = audioFile.audioHeader
        return AudioTrackSpec(
            durationMs = header.preciseTrackLength.times(1000).roundToLong(),
            bitRate = header.bitRateAsNumber,
            sampleRate = header.sampleRateAsNumber,
            type = type,
        )
    }

    private fun readAudioFile(file: File): AudioFile? {
        try {
            return AudioFileIO.readAs(file, "flac")
        } catch (_: Throwable) {
        }
        try {
            return AudioFileIO.readAs(file, "mp3")
        } catch (_: Throwable) {
        }
        return null
    }

    data class AudioTrackSpec(
        val durationMs: Long,
        val bitRate: Long,
        val sampleRate: Int,
        val type: AudioTrack.Type,
    )
}
package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.model.AudioTrack
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import java.io.File


class AudioTrackDecoderTest {

    private val tested = AudioTrackDecoder()

    @Test
    fun `decodes sample mp3`() {
        val file = File.createTempFile("asd","asd")
        file.delete()
        File(javaClass.classLoader.getResource("small.mp3")!!.file).copyTo(file)
        val decoded = tested.decode(file)
        assertThat(decoded).isNotNull
        assertThat(decoded!!.durationMs).isGreaterThan(0).isLessThan(60)
        assertThat(decoded.type).isEqualTo(AudioTrack.Type.MP3)
        assertThat(decoded.bitRate).isEqualTo(231)
        assertThat(decoded.sampleRate).isEqualTo(44100)
    }

    @Test
    fun `decodes sample flac`() {
        val file = File.createTempFile("asd","asd")
        file.delete()
        File(javaClass.classLoader.getResource("small.flac")!!.file).copyTo(file)
        val decoded = tested.decode(file)
        assertThat(decoded).isNotNull
        assertThat(decoded!!.durationMs).isGreaterThan(1000).isLessThan(1100)
        assertThat(decoded.type).isEqualTo(AudioTrack.Type.FLAC)
        assertThat(decoded.bitRate).isEqualTo(166)
        assertThat(decoded.sampleRate).isEqualTo(44100)
    }
}
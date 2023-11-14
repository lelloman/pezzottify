package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.utils.MP3_SAMPLE
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import java.io.File


class AudioTrackDecoderTest {

    private val tested = AudioTrackDecoder()

    @Test
    fun `decodes sample mp3`() {
        val file = File.createTempFile("sample", ".mp3")
        MP3_SAMPLE.inputStream().copyTo(file.outputStream())
        val decoded = tested.decode(file)
        assertThat(decoded).isNotNull
        assertThat(decoded!!.durationMs).isGreaterThan(0).isLessThan(60)
        assertThat(decoded.bitRate).isEqualTo(168)
        assertThat(decoded.sampleRate).isEqualTo(44100)
    }
}
package com.lelloman.pezzottify.android.player.equalizer

import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.util.UnstableApi
import com.lelloman.pezzottify.android.domain.equalizer.*
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.nio.ByteBuffer
import java.nio.ByteOrder

@UnstableApi
@RunWith(RobolectricTestRunner::class)
class EqualizerAudioProcessorTest {
    private val state = MutableStateFlow(EqualizerSettings())
    private val store = mockk<EqualizerStore> { every { this@mockk.state } returns this@EqualizerAudioProcessorTest.state }
    private val processor = EqualizerAudioProcessor(store)

    @Test fun `bypass consumes input and preserves signed PCM exactly`() {
        configure()
        val values = shortArrayOf(-32768, 32767, -1, 0, 12, -350)
        assertArrayEquals(values, process(values))
        assertTrue(processor.isActive)
        processor.queueEndOfStream()
        assertTrue(processor.isEnded)
    }

    @Test fun `settings change without reconfiguring and disabling restores bypass`() {
        configure()
        val input = ShortArray(12000) { if (it % 2 == 0) 10000 else -10000 }
        assertArrayEquals(input, process(input))
        state.value = EqualizerSettings(enabled = true, gains = List(5) { 12f })
        val filtered = process(input)
        assertFalse(input.contentEquals(filtered))
        assertTrue(filtered.all { kotlin.math.abs(it.toInt()) <= 32768 })
        state.value = state.value.copy(enabled = false)
        process(input) // finish crossfade
        assertArrayEquals(input, process(input))
    }

    @Test fun `flush resets filter history and handles sample rate and channel changes`() {
        state.value = EqualizerSettings(enabled = true, gains = List(5) { -6f })
        configure()
        process(ShortArray(12000) { 20000 })
        processor.flush()
        assertArrayEquals(ShortArray(100), process(ShortArray(100)))
        configure(8000, 1)
        assertArrayEquals(ShortArray(100), process(ShortArray(100)))
        processor.reset()
        configure(96000, 2)
        assertArrayEquals(ShortArray(100), process(ShortArray(100)))
    }

    @Test fun `buffers preserve channel alignment`() {
        state.value = EqualizerSettings(enabled = true, gains = List(5) { -6f })
        configure()
        repeat(100) {
            val output = process(shortArrayOf(12000, 0, -12000, 0))
            assertEquals(0, output[1].toInt())
            assertEquals(0, output[3].toInt())
        }
    }

    @Test fun `unsupported encoding is rejected rather than misinterpreted`() {
        assertThrows(AudioProcessor.UnhandledAudioFormatException::class.java) {
            processor.configure(AudioProcessor.AudioFormat(48000, 2, C.ENCODING_PCM_FLOAT))
        }
    }

    private fun configure(rate: Int = 48000, channels: Int = 2) {
        processor.configure(AudioProcessor.AudioFormat(rate, channels, C.ENCODING_PCM_16BIT))
        processor.flush()
    }

    private fun process(samples: ShortArray): ShortArray {
        val buffer = ByteBuffer.allocateDirect(samples.size * 2).order(ByteOrder.nativeOrder())
        samples.forEach(buffer::putShort)
        buffer.flip()
        processor.queueInput(buffer)
        assertFalse(buffer.hasRemaining())
        val output = processor.output.order(ByteOrder.nativeOrder())
        return ShortArray(output.remaining() / 2) { output.short }
    }
}

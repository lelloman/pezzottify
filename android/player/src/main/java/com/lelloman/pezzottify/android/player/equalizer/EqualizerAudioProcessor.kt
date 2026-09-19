package com.lelloman.pezzottify.android.player.equalizer

import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import com.lelloman.pezzottify.android.domain.equalizer.EqualizerStore
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

@UnstableApi
internal class EqualizerAudioProcessor(private val store: EqualizerStore) : BaseAudioProcessor() {
    private var engine: EqualizerEngine? = null

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        // Always remain in the chain, even when disabled, so toggling needs no player restart.
        return inputAudioFormat
    }

    override fun onFlush() {
        engine = if (inputAudioFormat.channelCount > 0 && inputAudioFormat.sampleRate > 0)
            EqualizerEngine(inputAudioFormat.sampleRate, inputAudioFormat.channelCount) else null
    }

    override fun onReset() { engine = null }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val processor = checkNotNull(engine)
        val settings = store.state.value
        processor.configure(settings.enabled, settings.gains)
        val output = replaceOutputBuffer(inputBuffer.remaining()).order(ByteOrder.nativeOrder())
        inputBuffer.order(ByteOrder.nativeOrder())
        while (inputBuffer.remaining() >= 2) {
            val sample = processor.process(inputBuffer.short.toDouble())
            output.putShort(sample.roundToInt().coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt()).toShort())
        }
        output.flip()
    }
}

package com.lelloman.pezzottify.android.player.equalizer

import android.content.Context
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.audio.*
import com.lelloman.pezzottify.android.domain.equalizer.EqualizerStore

@UnstableApi
internal class EqualizerRenderersFactory(context: Context, private val store: EqualizerStore) :
    DefaultRenderersFactory(context) {
    override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink {
        val sink = DefaultAudioSink.Builder(context)
            .setEnableFloatOutput(false)
            .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
            .setAudioProcessors(arrayOf(EqualizerAudioProcessor(store)))
            .build()
        // Encoded passthrough and hardware offload bypass PCM processors. Always decode locally,
        // including while disabled, so enabling EQ during a track takes effect immediately.
        return object : ForwardingAudioSink(sink) {
            override fun supportsFormat(format: Format) = getFormatSupport(format) != AudioSink.SINK_FORMAT_UNSUPPORTED
            override fun getFormatSupport(format: Format): Int =
                if (format.sampleMimeType == MimeTypes.AUDIO_RAW) super.getFormatSupport(format)
                else AudioSink.SINK_FORMAT_UNSUPPORTED
            override fun getFormatOffloadSupport(format: Format): AudioOffloadSupport = AudioOffloadSupport.DEFAULT_UNSUPPORTED
        }
    }
}

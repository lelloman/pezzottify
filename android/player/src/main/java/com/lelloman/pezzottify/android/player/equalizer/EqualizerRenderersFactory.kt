package com.lelloman.pezzottify.android.player.equalizer

import android.content.Context
import android.media.AudioTrack
import androidx.media3.common.AudioAttributes
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.audio.*
import com.lelloman.pezzottify.android.domain.equalizer.EqualizerStore

@UnstableApi
class EqualizerRenderersFactory(context: Context, private val store: EqualizerStore,
    private val outputs: AndroidEqualizerOutputController) :
    DefaultRenderersFactory(context) {
    override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioTrackPlaybackParams: Boolean): AudioSink {
        val owner = Any()
        var playing = false
        val sink = DefaultAudioSink.Builder(context)
            .setEnableFloatOutput(false)
            .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
            .setAudioProcessors(arrayOf(EqualizerAudioProcessor(store)))
            .setAudioTrackProvider(object : DefaultAudioSink.AudioTrackProvider {
                override fun getAudioTrack(config: AudioSink.AudioTrackConfig, attributes: AudioAttributes,
                    sessionId: Int, context: Context?): AudioTrack {
                    return DefaultAudioSink.AudioTrackProvider.DEFAULT.getAudioTrack(config, attributes, sessionId, context).also {
                        outputs.attach(owner, it)
                        outputs.setPlaying(owner, playing)
                    }
                }
            })
            .build()
        // Encoded passthrough and hardware offload bypass PCM processors. Always decode locally,
        // including while disabled, so enabling EQ during a track takes effect immediately.
        return object : ForwardingAudioSink(sink) {
            override fun play() {
                super.play()
                playing = true
                outputs.setPlaying(owner, true)
            }
            override fun pause() {
                super.pause()
                playing = false
                outputs.setPlaying(owner, false)
            }
            override fun flush() {
                outputs.detach(owner)
                super.flush()
            }
            override fun reset() {
                playing = false
                outputs.detach(owner)
                super.reset()
            }
            override fun supportsFormat(format: Format) = getFormatSupport(format) != AudioSink.SINK_FORMAT_UNSUPPORTED
            override fun getFormatSupport(format: Format): Int =
                if (format.sampleMimeType == MimeTypes.AUDIO_RAW) super.getFormatSupport(format)
                else AudioSink.SINK_FORMAT_UNSUPPORTED
            override fun getFormatOffloadSupport(format: Format): AudioOffloadSupport = AudioOffloadSupport.DEFAULT_UNSUPPORTED
        }
    }
}

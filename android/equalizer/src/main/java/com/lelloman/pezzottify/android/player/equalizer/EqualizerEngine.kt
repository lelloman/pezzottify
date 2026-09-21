package com.lelloman.pezzottify.android.player.equalizer

import com.lelloman.pezzottify.android.domain.equalizer.EqualizerBands
import kotlin.math.*

/** Audio-thread-only peaking filters (RBJ Audio EQ Cookbook), with independent channel history. */
internal class EqualizerEngine(private val sampleRate: Int, private val channels: Int) {
    private var gains = EqualizerBands.flat
    private var current = Bank(gains)
    private var previous: Bank? = null
    private val fadeFrames = (sampleRate / 50).coerceAtLeast(1) // 20 ms
    private var fadeFrame = fadeFrames
    private var channel = 0

    init { require(sampleRate > 0 && channels > 0) }

    fun configure(enabled: Boolean, requested: List<Float>) {
        EqualizerBands.validate(requested)
        val next = if (enabled) requested else EqualizerBands.flat
        if (next == gains || previous != null) return
        gains = next.toList()
        previous = current
        current = Bank(gains)
        fadeFrame = 0
    }

    fun process(sample: Double): Double {
        val next = current.process(sample, channel)
        val old = previous
        val output = if (old != null) {
            val mix = fadeFrame.toDouble() / fadeFrames
            old.process(sample, channel) * (1 - mix) + next * mix
        } else next
        channel = (channel + 1) % channels
        if (channel == 0 && old != null && ++fadeFrame >= fadeFrames) previous = null
        return output
    }

    private inner class Bank(bands: List<Float>) {
        private val filters = bands.mapIndexedNotNull { i, gain ->
            val frequency = EqualizerBands.frequencies[i].toDouble()
            if (gain == 0f || frequency >= sampleRate / 2.0) null else Filter(frequency, gain.toDouble())
        }.toTypedArray()
        // Measure the combined response, including overlap between bands. Add 0.5 dB of
        // margin when boosting. This avoids summing all boosts (which makes profiles too quiet).
        // Transients can still exceed full scale; the output processor saturates rather than wraps.
        private val headroom: Double = run {
            val frequencies = (0..512).map { sampleRate / 2.0 * it / 512 } +
                EqualizerBands.frequencies.filter { it < sampleRate / 2 }.map { it.toDouble() }
            val peak = frequencies.maxOf { hz -> filters.fold(1.0) { gain, filter -> gain * filter.magnitude(hz) } }
            if (peak > 1.00001) 1.0 / (peak * 10.0.pow(0.5 / 20)) else 1.0
        }
        fun process(input: Double, channel: Int): Double {
            var value = input * headroom
            for (filter in filters) value = filter.process(value, channel)
            return value
        }
    }

    private inner class Filter(frequency: Double, gainDb: Double) {
        private val a = 10.0.pow(gainDb / 40)
        private val omega = 2 * PI * frequency / sampleRate
        private val alpha = sin(omega) / 2 // Q = 1
        private val a0 = 1 + alpha / a
        private val b0 = (1 + alpha * a) / a0
        private val b1 = -2 * cos(omega) / a0
        private val b2 = (1 - alpha * a) / a0
        private val a1 = b1
        private val a2 = (1 - alpha / a) / a0
        private val z1 = DoubleArray(channels)
        private val z2 = DoubleArray(channels)
        fun magnitude(hz: Double): Double {
            val w = 2 * PI * hz / sampleRate
            val numerator = (b0 + b1 * cos(w) + b2 * cos(2 * w)).pow(2) +
                (b1 * sin(w) + b2 * sin(2 * w)).pow(2)
            val denominator = (1 + a1 * cos(w) + a2 * cos(2 * w)).pow(2) +
                (a1 * sin(w) + a2 * sin(2 * w)).pow(2)
            return sqrt(numerator / denominator)
        }
        fun process(input: Double, channel: Int): Double {
            val output = b0 * input + z1[channel]
            z1[channel] = b1 * input - a1 * output + z2[channel]
            z2[channel] = b2 * input - a2 * output
            return output
        }
    }
}

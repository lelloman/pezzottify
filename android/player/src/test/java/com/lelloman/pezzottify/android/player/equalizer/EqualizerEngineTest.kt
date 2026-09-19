package com.lelloman.pezzottify.android.player.equalizer

import com.lelloman.pezzottify.android.domain.equalizer.EqualizerBands
import org.junit.Assert.*
import org.junit.Test
import kotlin.math.*

class EqualizerEngineTest {
    @Test fun `disabled and flat are exact bypass`() {
        for (enabled in listOf(false, true)) {
            val engine = EqualizerEngine(48000, 2)
            engine.configure(enabled, if (enabled) EqualizerBands.flat else List(5) { 12f })
            listOf(-32768.0, -1.0, 0.0, 1.0, 32767.0).forEach { assertEquals(it, engine.process(it), 0.0) }
        }
    }

    @Test fun `cut is accurate at each center frequency`() {
        for (sampleRate in listOf(44100, 48000, 96000)) {
            EqualizerBands.frequencies.forEachIndexed { i, hz ->
                val gains = EqualizerBands.flat.toMutableList().also { it[i] = -12f }
                val ratio = response(sampleRate, hz, gains)
                assertEquals("$hz Hz at $sampleRate", -12.0, 20 * log10(ratio), 0.15)
            }
        }
    }

    @Test fun `boost has headroom but still changes relative frequency balance`() {
        val gains = listOf(12f, 0f, 0f, 0f, 0f)
        val center = response(48000, 60, gains)
        val elsewhere = response(48000, 3600, gains)
        assertTrue(center in 0.9..1.0)
        assertTrue(center / elsewhere > 3.8)
    }

    @Test fun `stereo history never leaks between channels`() {
        val engine = EqualizerEngine(48000, 2)
        engine.configure(true, listOf(-8f, 3f, 0f, -4f, 1f))
        repeat(10000) { i ->
            engine.process(if (i == 0) 20000.0 else 0.0)
            assertEquals(0.0, engine.process(0.0), 0.0)
        }
    }

    @Test fun `turning off returns to exact bypass after transition`() {
        val engine = EqualizerEngine(48000, 1)
        engine.configure(true, List(5) { -12f })
        repeat(2000) { engine.process(5000.0) }
        engine.configure(false, List(5) { -12f })
        repeat(2000) { engine.process(5000.0) }
        assertEquals(5000.0, engine.process(5000.0), 0.0)
    }

    @Test fun `low sample rates and extreme gains remain finite`() {
        for (rate in listOf(8000, 16000, 22050, 48000)) {
            val engine = EqualizerEngine(rate, 1)
            engine.configure(true, List(5) { if (it % 2 == 0) 12f else -12f })
            repeat(rate) { assertTrue(engine.process(sin(it.toDouble()) * 30000).isFinite()) }
        }
    }

    @Test fun `invalid gains are rejected`() {
        assertThrows(IllegalArgumentException::class.java) { EqualizerEngine(48000, 1).configure(true, listOf(1f)) }
        assertThrows(IllegalArgumentException::class.java) { EqualizerEngine(48000, 1).configure(true, List(5) { Float.NaN }) }
    }

    private fun response(rate: Int, frequency: Int, gains: List<Float>): Double {
        val engine = EqualizerEngine(rate, 1)
        engine.configure(true, gains)
        var inputPower = 0.0
        var outputPower = 0.0
        repeat(rate) { i ->
            val sample = 10000 * sin(2 * PI * frequency * i / rate)
            val output = engine.process(sample)
            if (i > rate / 2) { inputPower += sample * sample; outputPower += output * output }
        }
        return sqrt(outputPower / inputPower)
    }
}

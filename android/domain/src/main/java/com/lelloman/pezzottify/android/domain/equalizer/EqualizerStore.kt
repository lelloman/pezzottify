package com.lelloman.pezzottify.android.domain.equalizer

import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import kotlin.math.ln

object EqualizerBands {
    val frequencies = listOf(31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)
    val flat = List(frequencies.size) { 0f }
    const val MIN_DB = -12f
    const val MAX_DB = 12f
    fun validate(gains: List<Float>) {
        require(gains.size == frequencies.size && gains.all { it.isFinite() && it in MIN_DB..MAX_DB })
    }

    /** Preserve old profiles by interpolating dB on a logarithmic frequency axis.
     * This approximates the old curve, rather than guaranteeing identical filter response.
     */
    fun migrateLegacyGains(gains: List<Float>): List<Float> {
        val legacyFrequencies = listOf(60, 230, 910, 3600, 14000)
        if (gains.size == frequencies.size) return gains.also(::validate)
        require(gains.size == legacyFrequencies.size && gains.all { it.isFinite() && it in MIN_DB..MAX_DB })
        return frequencies.map { hz ->
            when {
                hz <= legacyFrequencies.first() -> gains.first()
                hz >= legacyFrequencies.last() -> gains.last()
                else -> {
                    val upper = legacyFrequencies.indexOfFirst { it >= hz }
                    val lowerHz = legacyFrequencies[upper - 1].toDouble()
                    val fraction = ln(hz / lowerHz) / ln(legacyFrequencies[upper] / lowerHz)
                    (gains[upper - 1] + fraction * (gains[upper] - gains[upper - 1])).toFloat()
                        .coerceIn(MIN_DB, MAX_DB)
                }
            }
        }
    }
}

@Serializable
data class EqualizerProfile(val id: String, val name: String, val gains: List<Float>)

@Serializable
data class EqualizerOutputAssociation(val outputKey: String, val outputName: String, val profileId: String)

data class EqualizerOutput(val key: String?, val name: String, val bluetooth: Boolean = false,
    val needsBluetoothPermission: Boolean = false)

/** Reports the actual playback route, or Android's anticipated media route while idle (API 33+). */
interface EqualizerOutputController {
    val output: StateFlow<EqualizerOutput?>
    /** Idempotent, process-lifetime observation independent of player and screen lifecycles. */
    fun start()
    fun refresh()
    /** Rechecks the live route before associating, to avoid a stale UI binding the wrong output. */
    fun associateCurrentOutput(profileId: String, expectedKey: String): Boolean
}

@Serializable
data class EqualizerSettings(
    val enabled: Boolean = false,
    val gains: List<Float> = EqualizerBands.flat,
    val profiles: List<EqualizerProfile> = emptyList(),
    val selectedProfileId: String? = null,
    val outputAssociations: List<EqualizerOutputAssociation> = emptyList(),
) {
    val selectedProfile get() = profiles.find { it.id == selectedProfileId }
    val isModified get() = selectedProfile?.let { it.gains != gains } ?: false
}

/** Device-local settings: output equipment profiles must not sync to other clients. */
interface EqualizerStore {
    val state: StateFlow<EqualizerSettings>
    fun setEnabled(enabled: Boolean)
    fun setBand(index: Int, gainDb: Float)
    fun reset()
    fun selectProfile(id: String)
    fun saveProfile(name: String)
    fun updateProfile()
    fun renameProfile(id: String, name: String)
    fun deleteProfile(id: String)
    fun associateOutput(outputKey: String, outputName: String, profileId: String)
    fun removeOutputAssociation(outputKey: String)
    fun applyOutput(outputKey: String?)
}

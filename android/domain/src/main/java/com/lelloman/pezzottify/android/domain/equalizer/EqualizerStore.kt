package com.lelloman.pezzottify.android.domain.equalizer

import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable

object EqualizerBands {
    val frequencies = listOf(60, 230, 910, 3600, 14000)
    val flat = List(frequencies.size) { 0f }
    const val MIN_DB = -12f
    const val MAX_DB = 12f
    fun validate(gains: List<Float>) {
        require(gains.size == frequencies.size && gains.all { it.isFinite() && it in MIN_DB..MAX_DB })
    }
}

@Serializable
data class EqualizerProfile(val id: String, val name: String, val gains: List<Float>)

@Serializable
data class EqualizerSettings(
    val enabled: Boolean = false,
    val gains: List<Float> = EqualizerBands.flat,
    val profiles: List<EqualizerProfile> = emptyList(),
    val selectedProfileId: String? = null,
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
}

package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import com.lelloman.pezzottify.android.domain.equalizer.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.util.UUID

internal class EqualizerStoreImpl(context: Context) : EqualizerStore {
    private val prefs = context.getSharedPreferences("Equalizer", Context.MODE_PRIVATE)
    private val json = Json { ignoreUnknownKeys = true }
    private val mutableState = MutableStateFlow(load())
    override val state = mutableState.asStateFlow()

    private fun load(): EqualizerSettings = runCatching {
        json.decodeFromString<EqualizerSettings>(prefs.getString("settings", null) ?: return EqualizerSettings())
            .also { settings ->
                EqualizerBands.validate(settings.gains)
                require(settings.profiles.size <= 50)
                require(settings.profiles.map { it.id }.distinct().size == settings.profiles.size)
                require(settings.profiles.map { it.name.lowercase(java.util.Locale.ROOT) }.distinct().size == settings.profiles.size)
                settings.profiles.forEach {
                    require(it.id.isNotBlank() && it.name.isNotBlank() && it.name.length <= 40)
                    EqualizerBands.validate(it.gains)
                }
                require(settings.selectedProfileId == null || settings.selectedProfile != null)
            }
    }.getOrElse { EqualizerSettings() }

    @Synchronized
    private fun change(transform: (EqualizerSettings) -> EqualizerSettings) {
        val next = transform(mutableState.value)
        prefs.edit().putString("settings", json.encodeToString(next)).apply()
        mutableState.value = next
    }

    private fun checkedName(state: EqualizerSettings, name: String, exceptId: String? = null): String {
        val trimmed = name.trim()
        require(trimmed.isNotEmpty() && trimmed.length <= 40)
        require(state.profiles.none { it.id != exceptId && it.name.equals(trimmed, ignoreCase = true) })
        return trimmed
    }

    override fun setEnabled(enabled: Boolean) = change { it.copy(enabled = enabled) }
    override fun setBand(index: Int, gainDb: Float) = change {
        require(index in EqualizerBands.frequencies.indices && gainDb.isFinite())
        it.copy(gains = it.gains.mapIndexed { i, gain ->
            if (i == index) gainDb.coerceIn(EqualizerBands.MIN_DB, EqualizerBands.MAX_DB) else gain
        })
    }
    override fun reset() = change { it.copy(gains = EqualizerBands.flat, selectedProfileId = null) }
    override fun selectProfile(id: String) = change {
        val profile = requireNotNull(it.profiles.find { p -> p.id == id })
        it.copy(gains = profile.gains.toList(), selectedProfileId = id)
    }
    override fun saveProfile(name: String) = change {
        require(it.profiles.size < 50)
        val profile = EqualizerProfile(UUID.randomUUID().toString(), checkedName(it, name), it.gains.toList())
        it.copy(profiles = it.profiles + profile, selectedProfileId = profile.id)
    }
    override fun updateProfile() = change {
        val selected = requireNotNull(it.selectedProfile)
        it.copy(profiles = it.profiles.map { p -> if (p.id == selected.id) p.copy(gains = it.gains.toList()) else p })
    }
    override fun renameProfile(id: String, name: String) = change {
        require(it.profiles.any { p -> p.id == id })
        val checked = checkedName(it, name, id)
        it.copy(profiles = it.profiles.map { p -> if (p.id == id) p.copy(name = checked) else p })
    }
    override fun deleteProfile(id: String) = change {
        it.copy(profiles = it.profiles.filterNot { p -> p.id == id },
            selectedProfileId = it.selectedProfileId.takeUnless { selected -> selected == id })
    }
}

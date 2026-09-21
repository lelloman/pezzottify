package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.lelloman.pezzottify.android.domain.equalizer.*
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class EqualizerStoreImplTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    @Before fun clear() { context.getSharedPreferences("Equalizer", Context.MODE_PRIVATE).edit().clear().commit() }

    @Test fun `defaults to disabled and flat`() {
        assertEquals(EqualizerSettings(), EqualizerStoreImpl(context).state.value)
    }

    @Test fun `output bindings persist and switch curves without turning EQ on`() {
        val store = EqualizerStoreImpl(context)
        store.saveProfile("Speaker")
        val speaker = store.state.value.selectedProfileId!!
        store.associateOutput("builtin:speaker", "Speaker", speaker)
        store.setBand(0, 6f)
        store.saveProfile("Sony")
        val sony = store.state.value.selectedProfileId!!
        store.associateOutput("bluetooth:AA:BB:CC:DD:EE:FF", "Sony headphones", sony)
        assertEquals(store.state.value, EqualizerStoreImpl(context).state.value)
        store.applyOutput("builtin:speaker")
        assertEquals(speaker, store.state.value.selectedProfileId)
        assertEquals(EqualizerBands.flat, store.state.value.gains)
        store.applyOutput("bluetooth:AA:BB:CC:DD:EE:FF")
        assertEquals(6f, store.state.value.gains[0])
        assertFalse(store.state.value.enabled)
        store.setEnabled(true)
        store.applyOutput("unknown")
        assertTrue(store.state.value.enabled)
        assertNull(store.state.value.selectedProfileId)
        assertEquals(EqualizerBands.flat, store.state.value.gains)
    }

    @Test fun `unassigned output resets manual curve even without any configured associations`() {
        val store = EqualizerStoreImpl(context)
        store.setEnabled(true)
        store.setBand(2, -5f)
        store.saveProfile("Manual")
        val saved = store.state.value.profiles
        store.setBand(2, 7f)
        store.applyOutput("builtin:speaker")
        assertEquals(EqualizerBands.flat, store.state.value.gains)
        assertNull(store.state.value.selectedProfileId)
        assertEquals(saved, store.state.value.profiles)
        assertTrue(store.state.value.enabled)
    }

    @Test fun `output changes discard unsaved adjustments without modifying saved profiles`() {
        val store = EqualizerStoreImpl(context)
        store.setBand(0, 6f)
        store.saveProfile("Sony")
        val sony = store.state.value.selectedProfileId!!
        store.associateOutput("sony", "Sony", sony)
        store.setBand(0, -8f)
        store.applyOutput("builtin:speaker")
        assertEquals(EqualizerBands.flat, store.state.value.gains)
        assertNull(store.state.value.selectedProfileId)
        store.setBand(0, 10f)
        store.applyOutput("sony")
        assertEquals(sony, store.state.value.selectedProfileId)
        assertEquals(6f, store.state.value.gains[0])
        assertEquals(6f, store.state.value.profiles.single().gains[0])
        assertFalse(store.state.value.isModified)
    }

    @Test fun `association replacement removal and profile deletion leave no dangling bindings`() {
        val store = EqualizerStoreImpl(context)
        store.saveProfile("One")
        val one = store.state.value.selectedProfileId!!
        store.associateOutput("device", "Device", one)
        store.saveProfile("Two")
        val two = store.state.value.selectedProfileId!!
        store.associateOutput("device", "Renamed device", two)
        assertEquals(two, store.state.value.outputAssociations.single().profileId)
        store.renameProfile(two, "New profile name")
        store.applyOutput("device")
        assertEquals("New profile name", store.state.value.selectedProfile!!.name)
        store.removeOutputAssociation("device")
        assertTrue(store.state.value.outputAssociations.isEmpty())
        store.associateOutput("device", "Device", two)
        store.deleteProfile(two)
        assertTrue(store.state.value.outputAssociations.isEmpty())
        assertEquals(store.state.value, EqualizerStoreImpl(context).state.value)
    }

    @Test fun `legacy profiles migrate without losing names selection or unsaved changes`() {
        context.getSharedPreferences("Equalizer", Context.MODE_PRIVATE).edit().putString("settings", """
            {"enabled":true,"gains":[12,6,0,-6,-12],"selectedProfileId":"sony",
             "profiles":[{"id":"sony","name":"Sony Headphones","gains":[6,6,6,6,6]},
                         {"id":"speaker","name":"Speaker","gains":[0,0,0,0,0]}]}
        """.trimIndent()).commit()
        val store = EqualizerStoreImpl(context)
        val migrated = store.state.value
        assertTrue(migrated.enabled)
        assertTrue(migrated.isModified)
        assertEquals("sony", migrated.selectedProfileId)
        assertEquals("Sony Headphones", migrated.selectedProfile!!.name)
        assertEquals(10, migrated.gains.size)
        assertEquals(12f, migrated.gains.first())
        assertEquals(-12f, migrated.gains.last())
        assertTrue(migrated.gains.zipWithNext().all { (left, right) -> left >= right })
        assertEquals(List(10) { 6f }, migrated.selectedProfile!!.gains)
        assertEquals(EqualizerBands.flat, migrated.profiles.last().gains)
        assertEquals(migrated, EqualizerStoreImpl(context).state.value)
        store.setBand(9, 2f) // Persist in the new format; no repeated interpolation on reload.
        assertEquals(store.state.value, EqualizerStoreImpl(context).state.value)
    }

    @Test fun `migration uses logarithmic frequency spacing and keeps an unmodified selection`() {
        val oldGains = listOf(0f, 12f, 0f, 0f, 0f)
        val migrated = EqualizerBands.migrateLegacyGains(oldGains)
        val expected = (12 * kotlin.math.ln(125.0 / 60) / kotlin.math.ln(230.0 / 60)).toFloat()
        assertEquals(expected, migrated[2], 0.0001f)
        context.getSharedPreferences("Equalizer", Context.MODE_PRIVATE).edit().putString("settings", """
            {"gains":[0,12,0,0,0],"selectedProfileId":"p",
             "profiles":[{"id":"p","name":"Profile","gains":[0,12,0,0,0]}]}
        """.trimIndent()).commit()
        val state = EqualizerStoreImpl(context).state.value
        assertFalse(state.isModified)
        assertEquals(migrated, state.gains)
        assertEquals(migrated, EqualizerBands.migrateLegacyGains(migrated))
    }

    @Test fun `settings and profiles survive recreation including unsaved adjustments`() {
        val store = EqualizerStoreImpl(context)
        store.setEnabled(true)
        store.setBand(0, 4.5f)
        store.saveProfile(" Sony Headphones ")
        store.setBand(1, -2f)
        assertEquals(store.state.value, EqualizerStoreImpl(context).state.value)
        assertEquals("Sony Headphones", store.state.value.selectedProfile?.name)
        assertTrue(store.state.value.isModified)
        assertEquals(0f, store.state.value.selectedProfile!!.gains[1])
    }

    @Test fun `profiles are independent and only update explicitly`() {
        val store = EqualizerStoreImpl(context)
        store.setBand(0, 6f)
        store.saveProfile("Sony Headphones")
        val sony = store.state.value.selectedProfileId!!
        store.setBand(0, -3f)
        store.saveProfile("Speaker")
        val speaker = store.state.value.selectedProfileId!!
        store.selectProfile(sony)
        assertEquals(6f, store.state.value.gains[0])
        store.setBand(0, 3f)
        store.updateProfile()
        assertFalse(store.state.value.isModified)
        store.selectProfile(speaker)
        assertEquals(-3f, store.state.value.gains[0])
        store.selectProfile(sony)
        assertEquals(3f, store.state.value.gains[0])
        assertFalse(store.state.value.enabled) // Loading does not unexpectedly turn processing on.
    }

    @Test fun `rename and delete preserve current audio`() {
        val store = EqualizerStoreImpl(context)
        store.setBand(3, 8f)
        store.saveProfile("Speaker")
        val id = store.state.value.selectedProfileId!!
        store.renameProfile(id, "Living room")
        assertEquals("Living room", store.state.value.selectedProfile!!.name)
        store.deleteProfile(id)
        assertNull(store.state.value.selectedProfileId)
        assertTrue(store.state.value.profiles.isEmpty())
        assertEquals(8f, store.state.value.gains[3])
        assertEquals(store.state.value, EqualizerStoreImpl(context).state.value)
    }

    @Test fun `flat does not destroy saved profiles or disable equalizer`() {
        val store = EqualizerStoreImpl(context)
        store.setEnabled(true)
        store.setBand(2, -6f)
        store.saveProfile("Voice")
        store.reset()
        assertEquals(EqualizerBands.flat, store.state.value.gains)
        assertTrue(store.state.value.enabled)
        assertNull(store.state.value.selectedProfileId)
        assertEquals(-6f, store.state.value.profiles.single().gains[2])
    }

    @Test fun `invalid input and duplicate names do not change state`() {
        val store = EqualizerStoreImpl(context)
        store.saveProfile("Speaker")
        val before = store.state.value
        listOf<() -> Unit>(
            { store.saveProfile(" speaker ") }, { store.saveProfile(" ") },
            { store.saveProfile("a".repeat(41)) }, { store.setBand(0, Float.NaN) },
            { store.setBand(-1, 1f) }, { store.selectProfile("missing") },
        ).forEach { invalid ->
            assertThrows(IllegalArgumentException::class.java) { invalid() }
            assertEquals(before, store.state.value)
        }
        store.setBand(0, 100f)
        assertEquals(EqualizerBands.MAX_DB, store.state.value.gains[0])
    }

    @Test fun `rename cannot overwrite another profile`() {
        val store = EqualizerStoreImpl(context)
        store.saveProfile("Speaker")
        store.saveProfile("Headphones")
        assertThrows(IllegalArgumentException::class.java) {
            store.renameProfile(store.state.value.selectedProfileId!!, "SPEAKER")
        }
        assertEquals(2, store.state.value.profiles.size)
    }

    @Test fun `corrupted or invalid stored state falls back safely`() {
        listOf("not json", "{\"enabled\":true,\"gains\":[50,0,0,0,0]}",
            "{\"gains\":[0]}", "{\"selectedProfileId\":\"missing\"}").forEach { value ->
            context.getSharedPreferences("Equalizer", Context.MODE_PRIVATE).edit().putString("settings", value).commit()
            assertEquals(EqualizerSettings(), EqualizerStoreImpl(context).state.value)
        }
    }
}

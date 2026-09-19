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

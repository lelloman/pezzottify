package com.lelloman.pezzottify.android.player.equalizer

import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioTrack
import com.lelloman.pezzottify.android.domain.equalizer.*
import io.mockk.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AndroidEqualizerOutputControllerTest {
    private val context = mockk<Context>(relaxed = true) {
        every { getString(any()) } returns "Device speaker"
        every { checkPermission(any(), any(), any()) } returns PackageManager.PERMISSION_GRANTED
    }
    private val settings = MutableStateFlow(EqualizerSettings(profiles = listOf(EqualizerProfile("p", "Profile", EqualizerBands.flat))))
    private val store = mockk<EqualizerStore>(relaxed = true) { every { state } returns settings }
    private val controller = AndroidEqualizerOutputController(context, store)
    private val track = mockk<AudioTrack>(relaxed = true)
    private val owner = Any()

    @Test fun `only a playing track produces a route and duplicates do not reload profiles`() {
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        controller.attach(owner, track)
        assertNull(controller.output.value)
        controller.setPlaying(owner, true)
        assertEquals("builtin:speaker", controller.output.value!!.key)
        controller.refresh()
        controller.setPlaying(owner, false)
        assertNull(controller.output.value)
        controller.setPlaying(owner, true)
        verify(exactly = 1) { store.applyOutput("builtin:speaker") }
        controller.detach(owner)
        assertNull(controller.output.value)
    }

    @Test fun `Bluetooth device changes switch profiles but display name changes do not`() {
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, "AA:BB:CC:DD:EE:FF", "Sony")
        controller.attach(owner, track)
        controller.setPlaying(owner, true)
        assertEquals("bluetooth:AA:BB:CC:DD:EE:FF", controller.output.value!!.key)
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, "AA:BB:CC:DD:EE:FF", "Renamed Sony")
        controller.refresh()
        verify(exactly = 1) { store.applyOutput("bluetooth:AA:BB:CC:DD:EE:FF") }
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, "AA:BB:CC:DD:EE:01", "VIKING")
        controller.refresh()
        verify { store.applyOutput("bluetooth:AA:BB:CC:DD:EE:01") }
        controller.detach(owner)
    }

    @Test fun `permission denial disables association and stale expected routes are rejected`() {
        every { context.checkPermission(any(), any(), any()) } returns PackageManager.PERMISSION_DENIED
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, "AA:BB:CC:DD:EE:FF")
        controller.attach(owner, track)
        controller.setPlaying(owner, true)
        assertTrue(controller.output.value!!.needsBluetoothPermission)
        assertNull(controller.output.value!!.key)
        assertFalse(controller.associateCurrentOutput("p", "bluetooth:AA:BB:CC:DD:EE:FF"))
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        assertFalse(controller.associateCurrentOutput("p", "bluetooth:AA:BB:CC:DD:EE:FF"))
        assertTrue(controller.associateCurrentOutput("p", "builtin:speaker"))
        verify(exactly = 1) { store.associateOutput("builtin:speaker", any(), "p") }
        controller.detach(owner)
    }

    @Test fun `stale owners cannot detach a newer track`() {
        every { track.routedDevice } returns device(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER)
        controller.attach(owner, track)
        controller.setPlaying(owner, true)
        controller.detach(Any())
        assertNotNull(controller.output.value)
        controller.detach(owner)
    }

    @Test fun `unstable or redacted identities are not used as persistent device keys`() {
        val keys = AndroidEqualizerOutputController
        assertNull(keys.outputKey(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, ""))
        assertNull(keys.outputKey(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, "02:00:00:00:00:00"))
        assertNull(keys.outputKey(AudioDeviceInfo.TYPE_WIRED_HEADPHONES, ""))
        assertEquals("bluetooth:AA:BB:CC:DD:EE:FF", keys.outputKey(AudioDeviceInfo.TYPE_BLE_HEADSET, "aa:bb:cc:dd:ee:ff"))
        assertEquals("builtin:speaker", keys.outputKey(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER_SAFE, ""))
    }

    private fun device(type: Int, address: String = "", name: String = "Output") = mockk<AudioDeviceInfo> {
        every { getType() } returns type
        every { getAddress() } returns address
        every { getProductName() } returns name
        every { getId() } returns type
    }
}

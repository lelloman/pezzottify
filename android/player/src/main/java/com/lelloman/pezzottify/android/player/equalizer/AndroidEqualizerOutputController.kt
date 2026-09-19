package com.lelloman.pezzottify.android.player.equalizer

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioRouting
import android.media.AudioTrack
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
import com.lelloman.pezzottify.android.domain.equalizer.*
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton
import java.util.Locale

/** Main-thread routing observer. Sink callbacks are marshalled from ExoPlayer's playback thread. */
@Singleton
class AndroidEqualizerOutputController @Inject constructor(
    @ApplicationContext private val context: Context,
    private val store: EqualizerStore,
) : EqualizerOutputController {
    private val handler = Handler(Looper.getMainLooper())
    private var owner: Any? = null
    private var track: AudioTrack? = null
    private var playing = false
    private var lastRoute: String? = null
    private val mutableOutput = MutableStateFlow<EqualizerOutput?>(null)
    override val output = mutableOutput.asStateFlow()
    private val listener = AudioRouting.OnRoutingChangedListener { refresh() }
    private val poll = object : Runnable {
        override fun run() {
            refresh()
            if (playing && track != null) handler.postDelayed(this, 1000)
        }
    }

    fun attach(owner: Any, audioTrack: AudioTrack) = onMain {
        track?.let { runCatching { it.removeOnRoutingChangedListener(listener) } }
        this.owner = owner
        track = audioTrack
        audioTrack.addOnRoutingChangedListener(listener, handler)
        refresh()
    }

    fun setPlaying(owner: Any, value: Boolean) = onMain {
        // play() can precede AudioTrack creation; the factory uses the same owner for attach.
        if (this.owner != null && this.owner !== owner) return@onMain
        this.owner = owner
        playing = value
        handler.removeCallbacks(poll)
        refresh()
        if (value) handler.post(poll)
    }

    fun detach(owner: Any) = onMain {
        if (this.owner !== owner) return@onMain
        track?.let { runCatching { it.removeOnRoutingChangedListener(listener) } }
        track = null
        this.owner = null
        playing = false
        mutableOutput.value = null
        handler.removeCallbacks(poll)
        // Keep lastRoute across track recreation/pause so manual edits are not repeatedly reset.
    }

    override fun refresh() {
        if (Looper.myLooper() != Looper.getMainLooper()) { handler.post { refresh() }; return }
        val active = track
        val devices = if (playing && active != null) runCatching {
            if (Build.VERSION.SDK_INT >= 36) active.routedDevices else listOfNotNull(active.routedDevice)
        }.getOrDefault(emptyList()) else emptyList()
        val result = when (devices.size) {
            0 -> null
            1 -> identify(devices.single())
            else -> EqualizerOutput(null, context.getString(com.lelloman.pezzottify.android.player.R.string.eq_multiple_outputs))
        }
        mutableOutput.value = result
        if (result != null) {
            val identity = result.key ?: "unidentified:${devices.map { it.id }.sorted()}"
            if (lastRoute != identity) {
                lastRoute = identity
                store.applyOutput(result.key)
            }
        }
    }

    override fun associateCurrentOutput(profileId: String, expectedKey: String): Boolean {
        check(Looper.myLooper() == Looper.getMainLooper())
        refresh()
        val current = output.value ?: return false
        val key = current.key ?: return false
        if (key != expectedKey) return false
        if (store.state.value.profiles.none { it.id == profileId }) return false
        store.associateOutput(key, current.name, profileId)
        return true
    }

    @SuppressLint("MissingPermission")
    private fun identify(device: AudioDeviceInfo): EqualizerOutput {
        val bluetooth = device.type in listOf(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
            AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLE_HEADSET,
            AudioDeviceInfo.TYPE_BLE_SPEAKER, AudioDeviceInfo.TYPE_HEARING_AID)
        val permission = Build.VERSION.SDK_INT < 31 || ContextCompat.checkSelfPermission(context,
            Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED
        val address = if (Build.VERSION.SDK_INT >= 28) runCatching { device.address }.getOrDefault("") else ""
        var name = device.productName.toString().ifBlank {
            context.getString(com.lelloman.pezzottify.android.player.R.string.eq_unknown_output)
        }
        if (bluetooth && permission && validBluetoothAddress(address)) {
            runCatching {
                val remote = context.getSystemService(BluetoothManager::class.java)?.adapter?.getRemoteDevice(address)
                (if (Build.VERSION.SDK_INT >= 30) remote?.alias else null) ?: remote?.name
            }.getOrNull()?.takeIf { it.isNotBlank() }?.let { name = it }
        }
        if (device.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER || device.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER_SAFE) {
            name = context.getString(com.lelloman.pezzottify.android.player.R.string.eq_device_speaker)
        }
        val key = outputKey(device.type, address, bluetooth && !permission)
        return EqualizerOutput(key, name, bluetooth, bluetooth && !permission)
    }

    private fun onMain(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) block() else handler.post(block)
    }

    internal companion object {
        fun validBluetoothAddress(address: String) = address.matches(Regex("(?i)([0-9a-f]{2}:){5}[0-9a-f]{2}")) &&
            address != "00:00:00:00:00:00" && address != "02:00:00:00:00:00"

        fun outputKey(type: Int, address: String, permissionMissing: Boolean = false): String? = when (type) {
            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, AudioDeviceInfo.TYPE_BUILTIN_SPEAKER_SAFE -> "builtin:speaker"
            AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "builtin:earpiece"
            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
            AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER, AudioDeviceInfo.TYPE_HEARING_AID ->
                address.takeIf { !permissionMissing && validBluetoothAddress(it) }?.let { "bluetooth:${it.uppercase(Locale.ROOT)}" }
            // Analog headphones have no device identity; do not silently bind all headsets together.
            else -> address.takeIf { it.isNotBlank() }?.let { "audio:$type:$it" }
        }
    }
}

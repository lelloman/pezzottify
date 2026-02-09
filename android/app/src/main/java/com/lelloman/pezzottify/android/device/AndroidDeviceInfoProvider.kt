package com.lelloman.pezzottify.android.device

import android.content.Context
import android.os.Build
import android.provider.Settings
import com.lelloman.pezzottify.android.BuildConfig
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AndroidDeviceInfoProvider @Inject constructor(
    @ApplicationContext private val context: Context,
) : DeviceInfoProvider {

    private val deviceUuid: String by lazy {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.getString(KEY_DEVICE_UUID, null) ?: run {
            val newUuid = "android-${UUID.randomUUID()}"
            prefs.edit().putString(KEY_DEVICE_UUID, newUuid).apply()
            newUuid
        }
    }

    override fun getDeviceInfo(): DeviceInfo = DeviceInfo(
        deviceUuid = deviceUuid,
        deviceType = BuildConfig.DEVICE_TYPE,
        deviceName = "${Build.MANUFACTURER} ${Build.MODEL}",
        osInfo = "Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
    )

    companion object {
        private const val PREFS_NAME = "pezzottify_device"
        private const val KEY_DEVICE_UUID = "device_uuid"
    }
}

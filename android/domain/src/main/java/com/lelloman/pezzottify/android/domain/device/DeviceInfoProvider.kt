package com.lelloman.pezzottify.android.domain.device

import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo

interface DeviceInfoProvider {
    fun getDeviceInfo(): DeviceInfo
}

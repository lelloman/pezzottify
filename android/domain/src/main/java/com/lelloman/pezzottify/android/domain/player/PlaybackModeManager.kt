package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

sealed interface PlaybackMode {
    data object Local : PlaybackMode
    data class Remote(val deviceId: Int, val deviceName: String) : PlaybackMode
}

@Singleton
class PlaybackModeManager @Inject constructor(
    loggerFactory: LoggerFactory,
) {
    private val logger = loggerFactory.getLogger(PlaybackModeManager::class)

    private val _mode = MutableStateFlow<PlaybackMode>(PlaybackMode.Local)
    val mode: StateFlow<PlaybackMode> = _mode.asStateFlow()

    fun enterRemoteMode(deviceId: Int, deviceName: String) {
        logger.info("Entering remote mode for device $deviceId ($deviceName)")
        _mode.value = PlaybackMode.Remote(deviceId, deviceName)
    }

    fun exitRemoteMode() {
        logger.info("Exiting remote mode")
        _mode.value = PlaybackMode.Local
    }
}

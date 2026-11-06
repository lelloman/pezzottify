package com.lelloman.pezzottify.android.ui.screen.main.devices

import android.content.Context
import com.lelloman.pezzottify.android.ui.R

/**
 * Maps a `playback.error` code from the server to a user-facing message.
 * Codes are listed in docs/multi-device-playback.md.
 */
fun playbackErrorMessage(context: Context, code: String, serverMessage: String): String = when (code) {
    "forbidden" -> context.getString(R.string.playback_error_forbidden)
    "device_not_found" -> context.getString(R.string.playback_error_device_not_found)
    "unknown_command" -> context.getString(R.string.playback_error_unknown_command)
    else -> serverMessage.ifBlank { context.getString(R.string.playback_error_generic) }
}

package com.lelloman.pezzottify.android.ui.screen.main.settings

import android.content.Intent

sealed interface SettingsScreenEvents {
    data class ShareLogs(val intent: Intent) : SettingsScreenEvents
}

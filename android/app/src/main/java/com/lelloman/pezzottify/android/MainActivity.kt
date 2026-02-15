package com.lelloman.pezzottify.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.notifications.AndroidSystemNotificationHelper
import com.lelloman.pezzottify.android.oidc.OidcCallbackHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import com.lelloman.pezzottify.android.ui.AppUi
import com.lelloman.pezzottify.android.mapping.toAppFontFamily
import com.lelloman.pezzottify.android.mapping.toColorPalette as toColorPalette
import com.lelloman.pezzottify.android.mapping.toThemeMode as toThemeMode
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
private typealias DomainThemeMode = com.lelloman.pezzottify.android.domain.settings.ThemeMode
private typealias UiColorPalette = com.lelloman.pezzottify.android.ui.theme.ColorPalette
private typealias DomainColorPalette = com.lelloman.pezzottify.android.domain.settings.ColorPalette
private typealias UiAppFontFamily = com.lelloman.pezzottify.android.ui.theme.AppFontFamily
private typealias DomainAppFontFamily = com.lelloman.pezzottify.android.domain.settings.AppFontFamily

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject
    lateinit var imageLoader: ImageLoader

    @Inject
    lateinit var userSettingsStore: UserSettingsStore

    @Inject
    lateinit var notificationRepository: NotificationRepository

    @Inject
    lateinit var oidcCallbackHandler: OidcCallbackHandler

    private val markAsReadScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Check for OIDC callback in initial intent
        handleOidcCallback(intent)

        // Mark internal notifications as read when launched from system notification click
        markNotificationsAsReadFromIntent(intent)

        setContent {
            setSingletonImageLoaderFactory { imageLoader }
            val themeMode by userSettingsStore.themeMode.collectAsState()
            val colorPalette by userSettingsStore.colorPalette.collectAsState()
            val fontFamily by userSettingsStore.fontFamily.collectAsState()
            AppUi(
                darkTheme = isSystemInDarkTheme(),
                themeMode = themeMode.toThemeMode(),
                colorPalette = colorPalette.toColorPalette(),
                fontFamily = fontFamily.toAppFontFamily(),
            )
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleOidcCallback(intent)
        markNotificationsAsReadFromIntent(intent)
    }

    private fun handleOidcCallback(intent: Intent) {
        if (oidcCallbackHandler.isOidcCallback(intent)) {
            oidcCallbackHandler.handleCallback(intent)
        }
    }

    private fun markNotificationsAsReadFromIntent(intent: Intent) {
        val notificationIds = intent.getStringArrayExtra(AndroidSystemNotificationHelper.EXTRA_NOTIFICATION_IDS)
            ?: return
        if (notificationIds.isEmpty()) return

        // Remove the extra so it doesn't get processed again on config change
        intent.removeExtra(AndroidSystemNotificationHelper.EXTRA_NOTIFICATION_IDS)

        markAsReadScope.launch {
            for (id in notificationIds) {
                notificationRepository.markAsRead(id)
            }
        }
    }
}

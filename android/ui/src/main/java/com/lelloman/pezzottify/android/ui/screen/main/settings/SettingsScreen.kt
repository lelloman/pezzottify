package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.screen.main.profile.CacheSettingsSection
import com.lelloman.pezzottify.android.ui.screen.main.profile.StorageInfoSection
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.theme.ThemeMode
import com.lelloman.pezzottify.android.ui.toLogViewer
import com.lelloman.pezzottify.android.ui.toStyleSettings
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow

@Composable
fun SettingsScreen(navController: NavController) {
    val viewModel = hiltViewModel<SettingsScreenViewModel>()
    SettingsScreenInternal(
        state = viewModel.state,
        events = viewModel.events,
        navController = navController,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScreenInternal(
    state: StateFlow<SettingsScreenState>,
    actions: SettingsScreenActions,
    events: Flow<SettingsScreenEvents>,
    navController: NavController,
) {
    val currentState by state.collectAsState()
    val context = LocalContext.current

    LaunchedEffect(Unit) {
        events.collect { event ->
            when (event) {
                is SettingsScreenEvents.ShareLogs -> {
                    context.startActivity(event.intent)
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_title)) },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
        ) {
            // Appearance Section
            Text(
                text = stringResource(R.string.appearance),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(modifier = Modifier.height(16.dp))

            // Appearance Settings Navigation Row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { navController.toStyleSettings() }
                    .padding(vertical = 12.dp),
                verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.theme_color_font),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = stringResource(R.string.customize_app_appearance),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                    contentDescription = stringResource(R.string.open_appearance_settings),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(24.dp)
                )
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            // Performance Section
            CacheSettingsSection(
                isCacheEnabled = currentState.isCacheEnabled,
                onCacheEnabledChanged = actions::setCacheEnabled
            )

            // Direct Downloads Section - only shown if user has permission
            if (currentState.hasIssueContentDownloadPermission) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

                DirectDownloadsSection(
                    isEnabled = currentState.directDownloadsEnabled,
                    hasPermission = currentState.hasIssueContentDownloadPermission,
                    onEnabledChanged = actions::setDirectDownloadsEnabled
                )
            }

            // External Search Section - only shown if user has RequestContent permission
            if (currentState.hasRequestContentPermission) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

                ExternalSearchSection(
                    isEnabled = currentState.externalSearchEnabled,
                    hasPermission = currentState.hasRequestContentPermission,
                    onEnabledChanged = actions::setExternalSearchEnabled
                )
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            FileLoggingSection(
                isEnabled = currentState.isFileLoggingEnabled,
                hasLogs = currentState.hasLogFiles,
                logSize = currentState.logFilesSize,
                onEnabledChanged = actions::setFileLoggingEnabled,
                onViewLogs = { navController.toLogViewer() },
                onShareLogs = actions::shareLogs,
                onClearLogs = actions::clearLogs,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            StorageInfoSection(
                storageInfo = currentState.storageInfo
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            ServerSection(
                baseUrl = currentState.baseUrl,
                baseUrlInput = currentState.baseUrlInput,
                baseUrlErrorRes = currentState.baseUrlErrorRes,
                isSaving = currentState.isBaseUrlSaving,
                onBaseUrlInputChanged = actions::onBaseUrlInputChanged,
                onSaveBaseUrl = actions::saveBaseUrl
            )

            Spacer(modifier = Modifier.height(24.dp))
        }
    }
}

@Composable
private fun SettingsLabel(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.primary
    )
}

@Composable
@Preview(showBackground = true)
private fun SettingsScreenPreview() {
    PezzottifyTheme {
        SettingsScreenInternal(
            state = MutableStateFlow(
                SettingsScreenState(
                    themeMode = ThemeMode.System,
                    colorPalette = ColorPalette.Classic,
                    fontFamily = AppFontFamily.System,
                    isCacheEnabled = true,
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            actions = object : SettingsScreenActions {
                override fun selectThemeMode(themeMode: ThemeMode) {}
                override fun selectColorPalette(colorPalette: ColorPalette) {}
                override fun selectFontFamily(fontFamily: AppFontFamily) {}
                override fun setCacheEnabled(enabled: Boolean) {}
                override fun setDirectDownloadsEnabled(enabled: Boolean) {}
                override fun setExternalSearchEnabled(enabled: Boolean) {}
                override fun setFileLoggingEnabled(enabled: Boolean) {}
                override fun shareLogs() {}
                override fun clearLogs() {}
                override fun onBaseUrlInputChanged(input: String) {}
                override fun saveBaseUrl() {}
            },
        )
    }
}

@Composable
@Preview(showBackground = true)
private fun SettingsScreenPreviewDark() {
    PezzottifyTheme(darkTheme = true, colorPalette = ColorPalette.PurpleHaze) {
        SettingsScreenInternal(
            state = MutableStateFlow(
                SettingsScreenState(
                    themeMode = ThemeMode.Dark,
                    colorPalette = ColorPalette.PurpleHaze,
                    fontFamily = AppFontFamily.Monospace,
                    isCacheEnabled = false,
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            actions = object : SettingsScreenActions {
                override fun selectThemeMode(themeMode: ThemeMode) {}
                override fun selectColorPalette(colorPalette: ColorPalette) {}
                override fun selectFontFamily(fontFamily: AppFontFamily) {}
                override fun setCacheEnabled(enabled: Boolean) {}
                override fun setDirectDownloadsEnabled(enabled: Boolean) {}
                override fun setExternalSearchEnabled(enabled: Boolean) {}
                override fun setFileLoggingEnabled(enabled: Boolean) {}
                override fun shareLogs() {}
                override fun clearLogs() {}
                override fun onBaseUrlInputChanged(input: String) {}
                override fun saveBaseUrl() {}
            },
        )
    }
}

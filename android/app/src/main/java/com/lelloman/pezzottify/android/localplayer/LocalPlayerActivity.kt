package com.lelloman.pezzottify.android.localplayer

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.localplayer.ui.LocalPlayerScreen
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.theme.ThemeMode
import dagger.hilt.android.AndroidEntryPoint

@AndroidEntryPoint
class LocalPlayerActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        val initialUris = parseIntent(intent)

        setContent {
            PezzottifyTheme(
                darkTheme = isSystemInDarkTheme(),
                themeMode = ThemeMode.System,
                colorPalette = ColorPalette.Classic
            ) {
                val viewModel: LocalPlayerViewModel = hiltViewModel()
                val state by viewModel.state.collectAsState()

                // Track states
                var hasLoadedInitial by remember { mutableStateOf(false) }
                var hasRequestedPermission by remember { mutableStateOf(false) }
                var hasMediaPermission by remember { mutableStateOf(hasMediaAudioPermission()) }

                // Permission request launcher
                val permissionLauncher = rememberLauncherForActivityResult(
                    contract = ActivityResultContracts.RequestPermission()
                ) { isGranted ->
                    hasMediaPermission = isGranted
                }

                // File picker launcher
                val filePickerLauncher = rememberLauncherForActivityResult(
                    contract = ActivityResultContracts.OpenMultipleDocuments()
                ) { uris: List<Uri> ->
                    if (uris.isNotEmpty()) {
                        // Take persistable permissions for each URI
                        uris.forEach { uri ->
                            try {
                                contentResolver.takePersistableUriPermission(
                                    uri,
                                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                                )
                            } catch (e: SecurityException) {
                                // Permission not granted, but we can still try to play
                            }
                        }

                        if (state.isEmpty) {
                            viewModel.loadFiles(uris)
                        } else {
                            viewModel.addToQueue(uris)
                        }
                    }
                }

                // Request media permission on first launch
                LaunchedEffect(Unit) {
                    if (!hasRequestedPermission && !hasMediaPermission) {
                        hasRequestedPermission = true
                        val permission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                            Manifest.permission.READ_MEDIA_AUDIO
                        } else {
                            Manifest.permission.READ_EXTERNAL_STORAGE
                        }
                        permissionLauncher.launch(permission)
                    }
                }

                // Load initial URIs or restore previous state
                LaunchedEffect(initialUris, hasLoadedInitial) {
                    if (!hasLoadedInitial) {
                        if (initialUris.isNotEmpty()) {
                            // New files to play
                            viewModel.loadFiles(initialUris)
                        } else {
                            // No new files - try to restore previous state
                            viewModel.tryRestoreState()
                        }
                        hasLoadedInitial = true
                    }
                }

                LocalPlayerScreen(
                    state = state,
                    onPlayPause = viewModel::togglePlayPause,
                    onSeek = viewModel::seekToPercent,
                    onSkipNext = viewModel::skipNext,
                    onSkipPrevious = viewModel::skipPrevious,
                    onSelectTrack = viewModel::selectTrack,
                    onAddFiles = {
                        filePickerLauncher.launch(arrayOf("audio/*"))
                    },
                    onClose = { finish() }
                )
            }
        }
    }

    private fun hasMediaAudioPermission(): Boolean {
        val permission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Manifest.permission.READ_MEDIA_AUDIO
        } else {
            Manifest.permission.READ_EXTERNAL_STORAGE
        }
        return ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        // The activity is already running, so we'll add files to the queue
        // We can't directly access the ViewModel here, so we'll use a different approach
        // For now, we'll just set the intent and let the Compose layer handle it
        // This requires a more complex state management which we'll add later
    }

    @Suppress("DEPRECATION")
    private fun parseIntent(intent: Intent): List<Uri> {
        val uris = mutableListOf<Uri>()

        when (intent.action) {
            Intent.ACTION_VIEW -> {
                // Single file opened
                intent.data?.let { uris.add(it) }
            }
            Intent.ACTION_SEND -> {
                // Single file shared
                val uri = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
                } else {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM)
                }
                uri?.let { uris.add(it) }
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                // Multiple files shared
                val extraUris = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
                } else {
                    intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)
                }
                extraUris?.let { uris.addAll(it) }
            }
        }

        return uris
    }
}

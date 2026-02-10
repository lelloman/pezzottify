package com.lelloman.pezzottify.android.ui.screen.tv

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.tv.fromTvSettingsToLogin
import com.lelloman.pezzottify.android.ui.tv.fromTvSettingsToNowPlaying

@Composable
fun TvSettingsScreen(navController: NavController) {
    val viewModel = hiltViewModel<TvSettingsViewModel>()
    val state by viewModel.state.collectAsState()
    var showLogoutConfirmation by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                TvSettingsEvent.NavigateToLogin -> navController.fromTvSettingsToLogin()
            }
        }
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        if (showLogoutConfirmation) {
            AlertDialog(
                onDismissRequest = { showLogoutConfirmation = false },
                title = { Text(stringResource(R.string.logout_confirmation_title)) },
                text = { Text(stringResource(R.string.logout_confirmation_message)) },
                confirmButton = {
                    TextButton(
                        onClick = {
                            showLogoutConfirmation = false
                            viewModel.clickOnLogout()
                        },
                        enabled = !state.isLoggingOut,
                    ) {
                        Text(stringResource(R.string.logout))
                    }
                },
                dismissButton = {
                    TextButton(
                        onClick = { showLogoutConfirmation = false },
                        enabled = !state.isLoggingOut,
                    ) {
                        Text(stringResource(R.string.cancel))
                    }
                },
            )
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(72.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = stringResource(R.string.settings),
                    style = MaterialTheme.typography.headlineLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }

            Card(
                modifier = Modifier
                    .fillMaxWidth(0.72f)
                    .weight(1f, fill = true)
                    .align(Alignment.CenterHorizontally),
                shape = RoundedCornerShape(24.dp),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(28.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Text(
                        text = stringResource(R.string.tv_settings_app_info),
                        style = MaterialTheme.typography.titleLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )

                    TvInfoRow(
                        label = stringResource(R.string.logged_in_as),
                        value = state.userHandle,
                    )
                    TvInfoRow(
                        label = stringResource(R.string.tv_settings_device),
                        value = stringResource(R.string.device_info, state.deviceName, state.deviceType),
                    )
                    TvInfoRow(
                        label = stringResource(R.string.tv_settings_connection),
                        value = state.connectionStatus,
                    )
                    TvInfoRow(
                        label = stringResource(R.string.server_url),
                        value = state.serverUrl,
                    )
                    TvInfoRow(
                        label = stringResource(R.string.server_version),
                        value = state.serverVersion,
                    )
                    TvInfoRow(
                        label = stringResource(R.string.version_label),
                        value = state.versionName,
                    )
                    TvInfoRow(
                        label = stringResource(R.string.git_commit),
                        value = state.gitCommit,
                    )
                }
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth(0.72f)
                    .align(Alignment.CenterHorizontally),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Button(
                    onClick = { navController.fromTvSettingsToNowPlaying() },
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.outlinedButtonColors(),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = null,
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    Text(stringResource(R.string.back))
                }

                Button(
                    onClick = { showLogoutConfirmation = true },
                    enabled = !state.isLoggingOut,
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.error,
                        contentColor = MaterialTheme.colorScheme.onError,
                    ),
                ) {
                    Text(
                        if (state.isLoggingOut) stringResource(R.string.logging_out)
                        else stringResource(R.string.logout),
                    )
                }
            }
        }
    }
}

@Composable
private fun TvInfoRow(
    label: String,
    value: String,
) {
    Column(
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

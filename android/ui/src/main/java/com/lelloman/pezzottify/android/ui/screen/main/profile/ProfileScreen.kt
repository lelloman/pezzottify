package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import com.lelloman.pezzottify.android.ui.fromProfileBackToLogin
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow

@Composable
fun ProfileScreen(navController: NavController) {
    val viewModel = hiltViewModel<ProfileScreenViewModel>()
    ProfileScreenInternal(
        state = viewModel.state,
        events = viewModel.events,
        navController = navController,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProfileScreenInternal(
    state: StateFlow<ProfileScreenState>,
    actions: ProfileScreenActions,
    events: Flow<ProfileScreenEvents>,
    navController: NavController,
) {
    val currentState by state.collectAsState()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                ProfileScreenEvents.NavigateToLoginScreen -> {
                    navController.fromProfileBackToLogin()
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Profile") }
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
            // User Info Section
            if (currentState.userName.isNotEmpty()) {
                SettingsLabel(text = "Logged in as")
                Text(
                    text = currentState.userName,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Spacer(modifier = Modifier.height(16.dp))
            }

            SettingsLabel(text = "Server URL")
            Text(
                text = currentState.baseUrl,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            // Settings Section
            Text(
                text = "Settings",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(modifier = Modifier.height(16.dp))

            // Play Behavior Setting
            SettingsLabel(text = "Track tap behavior")
            Spacer(modifier = Modifier.height(8.dp))
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                PlayBehavior.entries.forEachIndexed { index, playBehavior ->
                    SegmentedButton(
                        shape = SegmentedButtonDefaults.itemShape(
                            index = index,
                            count = PlayBehavior.entries.size
                        ),
                        onClick = { actions.selectPlayBehavior(playBehavior) },
                        selected = currentState.playBehavior == playBehavior
                    ) {
                        Text(
                            text = when (playBehavior) {
                                PlayBehavior.ReplacePlaylist -> "Replace"
                                PlayBehavior.AddToPlaylist -> "Add to queue"
                            },
                            maxLines = 1
                        )
                    }
                }
            }
            Text(
                text = when (currentState.playBehavior) {
                    PlayBehavior.ReplacePlaylist -> "Tapping a track replaces the current playlist"
                    PlayBehavior.AddToPlaylist -> "Tapping a track adds it to the current playlist"
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Theme Setting
            SettingsLabel(text = "Theme")
            Spacer(modifier = Modifier.height(8.dp))
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                ThemeMode.entries.forEachIndexed { index, themeMode ->
                    SegmentedButton(
                        shape = SegmentedButtonDefaults.itemShape(
                            index = index,
                            count = ThemeMode.entries.size
                        ),
                        onClick = { actions.selectThemeMode(themeMode) },
                        selected = currentState.themeMode == themeMode
                    ) {
                        Text(
                            text = when (themeMode) {
                                ThemeMode.System -> "System"
                                ThemeMode.Light -> "Light"
                                ThemeMode.Dark -> "Dark"
                            },
                            maxLines = 1
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.weight(1f))

            // Logout Button
            Button(
                onClick = actions::clickOnLogout,
                enabled = !currentState.isLoggingOut,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 16.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.error,
                    contentColor = MaterialTheme.colorScheme.onError
                )
            ) {
                Text(if (currentState.isLoggingOut) "Logging out..." else "Logout")
            }
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
private fun ProfileScreenPreview() {
    PezzottifyTheme {
        ProfileScreenInternal(
            state = MutableStateFlow(
                ProfileScreenState(
                    userName = "testuser@example.com",
                    baseUrl = "http://10.0.2.2:3001",
                    playBehavior = PlayBehavior.ReplacePlaylist,
                    themeMode = ThemeMode.System,
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            actions = object : ProfileScreenActions {
                override fun clickOnLogout() {}
                override fun selectPlayBehavior(playBehavior: PlayBehavior) {}
                override fun selectThemeMode(themeMode: ThemeMode) {}
            },
        )
    }
}

@Composable
@Preview(showBackground = true)
private fun ProfileScreenPreviewDark() {
    PezzottifyTheme(darkTheme = true) {
        ProfileScreenInternal(
            state = MutableStateFlow(
                ProfileScreenState(
                    userName = "testuser@example.com",
                    baseUrl = "http://10.0.2.2:3001",
                    playBehavior = PlayBehavior.AddToPlaylist,
                    themeMode = ThemeMode.Dark,
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            actions = object : ProfileScreenActions {
                override fun clickOnLogout() {}
                override fun selectPlayBehavior(playBehavior: PlayBehavior) {}
                override fun selectThemeMode(themeMode: ThemeMode) {}
            },
        )
    }
}

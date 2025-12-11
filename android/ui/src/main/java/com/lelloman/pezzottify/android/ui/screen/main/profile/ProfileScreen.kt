package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.model.Permission
import com.lelloman.pezzottify.android.ui.fromProfileBackToLogin
import com.lelloman.pezzottify.android.ui.toMyRequests
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow

@Composable
fun ProfileScreen(navController: NavController, rootNavController: NavController) {
    val viewModel = hiltViewModel<ProfileScreenViewModel>()
    ProfileScreenInternal(
        state = viewModel.state,
        events = viewModel.events,
        navController = navController,
        rootNavController = rootNavController,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun ProfileScreenInternal(
    state: StateFlow<ProfileScreenState>,
    actions: ProfileScreenActions,
    events: Flow<ProfileScreenEvents>,
    navController: NavController,
    rootNavController: NavController,
) {
    val currentState by state.collectAsState()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                ProfileScreenEvents.NavigateToLoginScreen -> {
                    rootNavController.fromProfileBackToLogin()
                }
            }
        }
    }

    if (currentState.showLogoutConfirmation) {
        AlertDialog(
            onDismissRequest = actions::dismissLogoutConfirmation,
            title = { Text("Logout") },
            text = { Text("Are you sure you want to logout?") },
            confirmButton = {
                TextButton(
                    onClick = actions::confirmLogout,
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error
                    )
                ) {
                    Text("Logout")
                }
            },
            dismissButton = {
                TextButton(onClick = actions::dismissLogoutConfirmation) {
                    Text("Cancel")
                }
            }
        )
    }

    // Permission description dialog
    currentState.selectedPermission?.let { permission ->
        AlertDialog(
            onDismissRequest = actions::onPermissionDialogDismissed,
            title = { Text(permission.displayName) },
            text = { Text(permission.description) },
            confirmButton = {
                TextButton(onClick = actions::onPermissionDialogDismissed) {
                    Text("OK")
                }
            }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Profile") },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                }
            )
        },
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
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

            // Permissions Section
            if (currentState.permissions.isNotEmpty()) {
                Spacer(modifier = Modifier.height(16.dp))
                SettingsLabel(text = "Permissions")
                Spacer(modifier = Modifier.height(8.dp))
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    currentState.permissions.sortedBy { it.ordinal }.forEach { permission ->
                        FilterChip(
                            selected = true,
                            onClick = { actions.onPermissionClicked(permission) },
                            label = { Text(permission.displayName) }
                        )
                    }
                }
            }

            // My Requests (only visible if user has RequestContent permission)
            if (currentState.permissions.contains(Permission.RequestContent)) {
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = { navController.toMyRequests() },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("My Requests")
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))

            // About Section
            Text(
                text = "About",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(modifier = Modifier.height(16.dp))

            SettingsLabel(text = "Version")
            Text(
                text = "${currentState.versionName} (${currentState.buildVariant})",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(8.dp))

            SettingsLabel(text = "Git Commit")
            Text(
                text = currentState.gitCommit,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(8.dp))

            SettingsLabel(text = "Server Version")
            Text(
                text = currentState.serverVersion,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

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
                    buildVariant = "debug",
                    versionName = "1.0",
                    gitCommit = "abc1234",
                    serverVersion = "0.5.0 (abc1234)",
                    permissions = setOf(
                        Permission.AccessCatalog,
                        Permission.LikeContent,
                        Permission.OwnPlaylists,
                    ),
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            rootNavController = rememberNavController(),
            actions = object : ProfileScreenActions {
                override fun clickOnLogout() {}
                override fun confirmLogout() {}
                override fun dismissLogoutConfirmation() {}
                override fun onPermissionClicked(permission: Permission) {}
                override fun onPermissionDialogDismissed() {}
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
                    buildVariant = "release",
                    versionName = "1.0",
                    gitCommit = "def5678",
                    serverVersion = "disconnected",
                    permissions = setOf(
                        Permission.AccessCatalog,
                        Permission.LikeContent,
                        Permission.OwnPlaylists,
                        Permission.EditCatalog,
                        Permission.ManagePermissions,
                    ),
                )
            ),
            events = flow {},
            navController = rememberNavController(),
            rootNavController = rememberNavController(),
            actions = object : ProfileScreenActions {
                override fun clickOnLogout() {}
                override fun confirmLogout() {}
                override fun dismissLogoutConfirmation() {}
                override fun onPermissionClicked(permission: Permission) {}
                override fun onPermissionDialogDismissed() {}
            },
        )
    }
}

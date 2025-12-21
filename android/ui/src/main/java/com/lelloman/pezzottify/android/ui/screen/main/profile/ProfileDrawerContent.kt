package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ExitToApp
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.NewReleases
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Queue
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.SmallNotificationBadge
import com.lelloman.pezzottify.android.ui.model.Permission
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

/**
 * Profile content displayed in a navigation drawer.
 */
@Composable
fun ProfileDrawerContent(
    onNavigateToProfile: () -> Unit,
    onNavigateToMyRequests: () -> Unit,
    onNavigateToNotifications: () -> Unit,
    onNavigateToListeningHistory: () -> Unit,
    onNavigateToWhatsNew: () -> Unit,
    onNavigateToAbout: () -> Unit,
    onNavigateToLogin: () -> Unit,
    onCloseDrawer: () -> Unit,
    notificationUnreadCount: Int = 0,
) {
    val viewModel = hiltViewModel<ProfileScreenViewModel>()
    ProfileDrawerContentInternal(
        state = viewModel.state,
        events = viewModel.events,
        actions = viewModel,
        onNavigateToProfile = onNavigateToProfile,
        onNavigateToMyRequests = onNavigateToMyRequests,
        onNavigateToNotifications = onNavigateToNotifications,
        onNavigateToListeningHistory = onNavigateToListeningHistory,
        onNavigateToWhatsNew = onNavigateToWhatsNew,
        onNavigateToAbout = onNavigateToAbout,
        onNavigateToLogin = onNavigateToLogin,
        onCloseDrawer = onCloseDrawer,
        notificationUnreadCount = notificationUnreadCount,
    )
}

@Composable
private fun ProfileDrawerContentInternal(
    state: StateFlow<ProfileScreenState>,
    actions: ProfileScreenActions,
    events: Flow<ProfileScreenEvents>,
    onNavigateToProfile: () -> Unit,
    onNavigateToMyRequests: () -> Unit,
    onNavigateToNotifications: () -> Unit,
    onNavigateToListeningHistory: () -> Unit,
    onNavigateToWhatsNew: () -> Unit,
    onNavigateToAbout: () -> Unit,
    onNavigateToLogin: () -> Unit,
    onCloseDrawer: () -> Unit,
    notificationUnreadCount: Int,
) {
    val currentState by state.collectAsState()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                ProfileScreenEvents.NavigateToLoginScreen -> {
                    onNavigateToLogin()
                }
            }
        }
    }

    // Logout confirmation dialog
    if (currentState.showLogoutConfirmation) {
        AlertDialog(
            onDismissRequest = actions::dismissLogoutConfirmation,
            title = { Text(stringResource(R.string.logout_confirmation_title)) },
            text = { Text(stringResource(R.string.logout_confirmation_message)) },
            confirmButton = {
                TextButton(
                    onClick = actions::confirmLogout,
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error
                    )
                ) {
                    Text(stringResource(R.string.logout))
                }
            },
            dismissButton = {
                TextButton(onClick = actions::dismissLogoutConfirmation) {
                    Text(stringResource(R.string.cancel))
                }
            }
        )
    }

    // Permission description dialog
    currentState.selectedPermission?.let { permission ->
        AlertDialog(
            onDismissRequest = actions::onPermissionDialogDismissed,
            title = { Text(stringResource(permission.displayNameRes)) },
            text = { Text(stringResource(permission.descriptionRes)) },
            confirmButton = {
                TextButton(onClick = actions::onPermissionDialogDismissed) {
                    Text(stringResource(R.string.ok))
                }
            }
        )
    }

    ModalDrawerSheet(
        modifier = Modifier
            .width(300.dp)
            .fillMaxHeight()
    ) {
        Column(
            modifier = Modifier
                .fillMaxHeight()
                .verticalScroll(rememberScrollState())
        ) {
            // Header with user avatar, name and "View profile"
            DrawerHeader(
                userName = currentState.userName,
                onViewProfile = {
                    onCloseDrawer()
                    onNavigateToProfile()
                }
            )

            HorizontalDivider()

            Spacer(modifier = Modifier.height(8.dp))

            // Notifications
            DrawerMenuItemWithBadge(
                icon = Icons.Outlined.Notifications,
                label = stringResource(R.string.notifications),
                badgeCount = notificationUnreadCount,
                onClick = {
                    onCloseDrawer()
                    onNavigateToNotifications()
                }
            )

            // My Requests (only visible if user has RequestContent permission)
            if (currentState.permissions.contains(Permission.RequestContent)) {
                DrawerMenuItem(
                    icon = Icons.Outlined.Queue,
                    label = stringResource(R.string.my_requests_title),
                    onClick = {
                        onCloseDrawer()
                        onNavigateToMyRequests()
                    }
                )
            }

            // Listening History
            DrawerMenuItem(
                icon = Icons.Outlined.History,
                label = stringResource(R.string.listening_history_title),
                onClick = {
                    onCloseDrawer()
                    onNavigateToListeningHistory()
                }
            )

            Spacer(modifier = Modifier.height(8.dp))
            HorizontalDivider()
            Spacer(modifier = Modifier.height(8.dp))

            // What's New
            DrawerMenuItem(
                icon = Icons.Outlined.NewReleases,
                label = stringResource(R.string.whats_new_header),
                onClick = {
                    onCloseDrawer()
                    onNavigateToWhatsNew()
                }
            )

            // About
            DrawerMenuItem(
                icon = Icons.Outlined.Info,
                label = stringResource(R.string.about),
                onClick = {
                    onCloseDrawer()
                    onNavigateToAbout()
                }
            )

            Spacer(modifier = Modifier.weight(1f))

            HorizontalDivider()

            // Logout
            DrawerMenuItem(
                icon = Icons.AutoMirrored.Outlined.ExitToApp,
                label = if (currentState.isLoggingOut) {
                    stringResource(R.string.logging_out)
                } else {
                    stringResource(R.string.logout)
                },
                onClick = { if (!currentState.isLoggingOut) actions.clickOnLogout() }
            )

            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

@Composable
private fun DrawerHeader(
    userName: String,
    onViewProfile: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onViewProfile)
            .padding(horizontal = 16.dp, vertical = 24.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically
        ) {
            // User avatar with initials
            UserAvatarLarge(userName = userName)

            Spacer(modifier = Modifier.width(16.dp))

            Column {
                Text(
                    text = userName.ifEmpty { "User" },
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = stringResource(R.string.view_profile),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun UserAvatarLarge(userName: String) {
    val initials = extractInitials(userName)

    Surface(
        modifier = Modifier.size(56.dp),
        shape = CircleShape,
        color = MaterialTheme.colorScheme.primaryContainer
    ) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier.fillMaxSize()
        ) {
            Text(
                text = initials,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
        }
    }
}

private fun extractInitials(userName: String): String {
    if (userName.isEmpty()) return "?"

    // If it's an email, take the part before @
    val nameToUse = if (userName.contains("@")) {
        userName.substringBefore("@")
    } else {
        userName
    }

    // Split by common delimiters (space, dot, underscore, dash)
    val parts = nameToUse.split(" ", ".", "_", "-").filter { it.isNotEmpty() }

    return when {
        parts.isEmpty() -> userName.take(1).uppercase()
        parts.size == 1 -> parts[0].take(2).uppercase()
        else -> (parts[0].take(1) + parts[1].take(1)).uppercase()
    }
}

@Composable
private fun DrawerMenuItem(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
) {
    NavigationDrawerItem(
        icon = {
            Icon(
                imageVector = icon,
                contentDescription = null
            )
        },
        label = { Text(label) },
        selected = false,
        onClick = onClick,
        modifier = Modifier.padding(horizontal = 12.dp)
    )
}

@Composable
private fun DrawerMenuItemWithBadge(
    icon: ImageVector,
    label: String,
    badgeCount: Int,
    onClick: () -> Unit,
) {
    NavigationDrawerItem(
        icon = {
            Box {
                Icon(
                    imageVector = icon,
                    contentDescription = null
                )
                SmallNotificationBadge(
                    count = badgeCount,
                    modifier = Modifier.align(Alignment.TopEnd)
                )
            }
        },
        label = { Text(label) },
        selected = false,
        onClick = onClick,
        modifier = Modifier.padding(horizontal = 12.dp)
    )
}

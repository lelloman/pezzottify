package com.lelloman.pezzottify.android.ui.screen.maintenance

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.fromMaintenanceToLogin
import com.lelloman.pezzottify.android.ui.fromMaintenanceToMain
import java.text.DateFormat
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone

@Composable
fun MaintenanceScreen(
    navController: NavController,
    onMaintenanceEnded: ((Boolean) -> Unit)? = null,
    onLoggedOut: (() -> Unit)? = null,
) {
    val viewModel = hiltViewModel<MaintenanceViewModel>()
    val state by viewModel.state.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                is MaintenanceScreenEvent.MaintenanceEnded -> {
                    onMaintenanceEnded?.invoke(event.isLoggedIn) ?: if (event.isLoggedIn) {
                        navController.fromMaintenanceToMain()
                    } else {
                        navController.fromMaintenanceToLogin()
                    }
                }

                MaintenanceScreenEvent.LoggedOut ->
                    onLoggedOut?.invoke() ?: navController.fromMaintenanceToLogin()
            }
        }
    }

    MaintenanceScreenInternal(
        state = state,
        onRetry = viewModel::refresh,
        onLogout = viewModel::logout,
    )
}

@Composable
internal fun MaintenanceScreenInternal(
    state: MaintenanceScreenState,
    onRetry: () -> Unit,
    onLogout: () -> Unit,
) {
    val eta = remember(state.estimatedEnd) { formatEstimatedEnd(state.estimatedEnd) }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = stringResource(R.string.maintenance_title),
                style = MaterialTheme.typography.headlineMedium,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = state.message
                    ?: stringResource(R.string.maintenance_default_message),
                style = MaterialTheme.typography.bodyLarge,
                textAlign = TextAlign.Center,
            )
            if (eta != null) {
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = stringResource(R.string.maintenance_expected_back, eta),
                    style = MaterialTheme.typography.bodyMedium,
                    textAlign = TextAlign.Center,
                )
            }
            if (state.error) {
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = stringResource(R.string.maintenance_status_unavailable),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                    textAlign = TextAlign.Center,
                )
            }
            Spacer(modifier = Modifier.height(32.dp))
            Button(onClick = onRetry, enabled = !state.isLoading && !state.isLoggingOut) {
                Text(stringResource(R.string.retry))
            }
            if (state.isLoggedIn) {
                Spacer(modifier = Modifier.height(8.dp))
                TextButton(onClick = onLogout, enabled = !state.isLoggingOut) {
                    Text(
                        stringResource(
                            if (state.isLoggingOut) R.string.logging_out else R.string.logout,
                        ),
                    )
                }
            }
            if (state.isLoading) {
                Spacer(modifier = Modifier.height(24.dp))
                CircularProgressIndicator()
            }
        }
    }
}

private fun formatEstimatedEnd(raw: String?): String? {
    if (raw.isNullOrBlank()) return null
    return try {
        val parser = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply {
            timeZone = TimeZone.getTimeZone("UTC")
        }
        val date = parser.parse(raw) ?: return raw
        DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(date)
    } catch (_: Exception) {
        raw
    }
}

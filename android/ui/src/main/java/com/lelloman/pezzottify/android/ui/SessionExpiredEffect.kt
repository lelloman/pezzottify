package com.lelloman.pezzottify.android.ui

import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.navigation.NavController
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject

/**
 * Composable effect that observes session expired events and handles them.
 * When a session expiration is detected:
 * 1. Shows a toast message to the user
 * 2. Performs cleanup (logout without calling server)
 * 3. Navigates to the login screen
 */
@Composable
fun SessionExpiredEffect(navController: NavController) {
    val viewModel = hiltViewModel<SessionExpiredViewModel>()
    val context = LocalContext.current

    LaunchedEffect(Unit) {
        viewModel.sessionExpiredEvents.collect {
            // Show toast to inform user
            Toast.makeText(
                context,
                "Session expired. Please log in again.",
                Toast.LENGTH_LONG
            ).show()

            // Perform cleanup and wait for it to complete before navigating
            viewModel.handleSessionExpired()

            // Navigate to login after cleanup is done
            navController.fromMainBackToLogin()
        }
    }
}

@HiltViewModel
class SessionExpiredViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    val sessionExpiredEvents: Flow<Unit> = interactor.sessionExpiredEvents()

    /**
     * Performs cleanup when session has expired.
     * This is a suspend function to ensure cleanup completes before navigation.
     */
    suspend fun handleSessionExpired() {
        interactor.handleSessionExpired()
    }

    interface Interactor {
        fun sessionExpiredEvents(): Flow<Unit>
        suspend fun handleSessionExpired()
    }
}

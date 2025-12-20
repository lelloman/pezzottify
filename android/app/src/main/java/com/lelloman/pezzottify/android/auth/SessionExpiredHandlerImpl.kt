package com.lelloman.pezzottify.android.auth

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredEventBus
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Implementation of [SessionExpiredHandler] that emits an event to the [SessionExpiredEventBus].
 * This avoids circular dependencies by not directly calling PerformLogout.
 * The UI layer observes the event bus and handles the actual logout.
 *
 * Only emits if the user is currently logged in, preventing duplicate events
 * after logout has already occurred.
 */
@Singleton
class SessionExpiredHandlerImpl @Inject constructor(
    private val sessionExpiredEventBus: SessionExpiredEventBus,
    private val authStore: AuthStore,
) : SessionExpiredHandler {

    override fun onSessionExpired() {
        // Only emit if currently logged in - prevents duplicate events
        // after the first one has already triggered logout
        if (authStore.getAuthState().value is AuthState.LoggedIn) {
            sessionExpiredEventBus.emit()
        }
    }
}

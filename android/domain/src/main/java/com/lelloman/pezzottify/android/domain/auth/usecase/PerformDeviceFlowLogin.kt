package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import javax.inject.Inject

/**
 * Completes a device-flow (RFC 8628) sign-in once the user approved the code on another device.
 *
 * The approved tokens are the same kind the browser flow returns for the same client id, so the
 * session is stored and initialised by [PerformOidcLogin]: same [AuthState.LoggedIn] shape, same
 * WebSocket connection and sync start, same refresh path afterwards.
 */
class PerformDeviceFlowLogin @Inject constructor(
    private val performOidcLogin: PerformOidcLogin,
) : UseCase() {

    suspend operator fun invoke(approved: DeviceFlowAuthManager.PollResult.Approved): PerformOidcLogin.LoginResult =
        performOidcLogin(
            OidcAuthManager.AuthorizationResult.Success(
                idToken = approved.idToken,
                refreshToken = approved.refreshToken,
                userHandle = approved.userHandle,
            )
        )
}

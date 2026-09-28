package com.lelloman.pezzottify.android.oidc

import com.google.common.truth.Truth.assertThat
import net.openid.appauth.AuthorizationException
import org.junit.Test

class RefreshCredentialRejectionTest {
    @Test
    fun `only invalid grant requires signing in again`() {
        assertThat(AuthorizationException.TokenRequestErrors.INVALID_GRANT.rejectsRefreshCredential()).isTrue()
        listOf(
            AuthorizationException.GeneralErrors.NETWORK_ERROR,
            AuthorizationException.GeneralErrors.SERVER_ERROR,
            AuthorizationException.GeneralErrors.JSON_DESERIALIZATION_ERROR,
            AuthorizationException.TokenRequestErrors.INVALID_CLIENT,
            AuthorizationException.TokenRequestErrors.INVALID_REQUEST,
        ).forEach { assertThat(it.rejectsRefreshCredential()).isFalse() }
    }
}

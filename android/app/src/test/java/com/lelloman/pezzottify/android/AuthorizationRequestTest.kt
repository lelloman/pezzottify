package com.lelloman.pezzottify.android

import com.google.common.truth.Truth.assertThat
import okhttp3.Request
import org.junit.Test

class AuthorizationRequestTest {

    @Test
    fun `bearer authorization replaces an existing header`() {
        val request = Request.Builder()
            .url("https://pezzottify.example/v1/content/image/album-id")
            .addHeader("Authorization", "Bearer stale-or-duplicate-token")
            .build()

        val authenticatedRequest = request.withBearerAuthorization("current-token")

        assertThat(authenticatedRequest.headers("Authorization"))
            .containsExactly("Bearer current-token")
    }
}

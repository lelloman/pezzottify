package com.lelloman.pezzottify.android.domain.auth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class AuthorizationHeaderTest {

    @Test
    fun `formats opaque and OIDC tokens as Bearer credentials`() {
        assertEquals("Bearer opaque-token", bearerAuthorization("opaque-token"))
        assertEquals(
            "Bearer header.payload.signature",
            bearerAuthorization("header.payload.signature")
        )
    }

    @Test
    fun `rejects blank tokens`() {
        assertThrows(IllegalArgumentException::class.java) {
            bearerAuthorization("  ")
        }
    }
}

package com.lelloman.pezzottify.android.oidc

import com.google.common.truth.Truth.assertThat
import org.junit.Test
import java.util.Base64

class IdTokenClaimsTest {

    @Test
    fun `prefers preferred_username over email and sub`() {
        assertThat(userHandleFromIdToken(jwt("""{"sub":"42","email":"a@b.c","preferred_username":"alice"}""")))
            .isEqualTo("alice")
    }

    @Test
    fun `falls back to email then sub`() {
        assertThat(userHandleFromIdToken(jwt("""{"sub":"42","email":"a@b.c"}"""))).isEqualTo("a@b.c")
        assertThat(userHandleFromIdToken(jwt("""{"sub":"42","preferred_username":""}"""))).isEqualTo("42")
    }

    @Test
    fun `handles url-safe base64 characters`() {
        // "~~~" and "???" encode to URL-safe '-' and '_' characters.
        assertThat(userHandleFromIdToken(jwt("""{"preferred_username":"~~~???"}"""))).isEqualTo("~~~???")
    }

    @Test
    fun `malformed tokens fall back to user`() {
        assertThat(userHandleFromIdToken("not-a-jwt")).isEqualTo("user")
        assertThat(userHandleFromIdToken("a.!!!.c")).isEqualTo("user")
        assertThat(userHandleFromIdToken(jwt("""{"exp":1}"""))).isEqualTo("user")
    }

    private fun jwt(payload: String): String {
        val encoder = Base64.getUrlEncoder().withoutPadding()
        return encoder.encodeToString("""{"alg":"RS256"}""".toByteArray()) + "." +
            encoder.encodeToString(payload.toByteArray()) + ".sig"
    }
}

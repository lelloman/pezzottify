package com.lelloman.pezzottify.android.oidc

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import okio.ByteString.Companion.decodeBase64

/**
 * The user handle shown for an OIDC session, read from the (already issuer-validated) ID token:
 * `preferred_username`, then `email`, then `sub`, falling back to "user".
 *
 * Shared by the browser (authorization-code) and the device flow so both name the session the same way.
 */
internal fun userHandleFromIdToken(idToken: String): String {
    val claims = runCatching {
        val parts = idToken.split(".")
        if (parts.size != 3) return@runCatching null
        // Okio accepts both the standard and the URL-safe alphabet, with or without padding.
        val payload = parts[1].decodeBase64()?.utf8() ?: return@runCatching null
        Json.parseToJsonElement(payload).jsonObject
    }.getOrNull() ?: return DEFAULT_USER_HANDLE

    return listOf("preferred_username", "email", "sub")
        .firstNotNullOfOrNull { claim ->
            (claims[claim] as? JsonPrimitive)?.takeIf { it.isString }?.content?.takeIf { it.isNotBlank() }
        }
        ?: DEFAULT_USER_HANDLE
}

private const val DEFAULT_USER_HANDLE = "user"

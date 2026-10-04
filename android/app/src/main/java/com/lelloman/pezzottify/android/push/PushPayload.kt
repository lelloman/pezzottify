package com.lelloman.pezzottify.android.push

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** What a decrypted push message asks for (docs/unifiedpush.md). */
sealed interface PushPayload {

    /** Wake up and run the sync catch-up: `sync` messages and anything unrecognised. */
    data object Wake : PushPayload

    /** An administrator's test notification, shown as is. */
    data class Test(val title: String, val body: String) : PushPayload

    companion object {
        const val DEFAULT_TEST_TITLE = "Test notification"
        private const val MAX_TITLE_CHARS = 100
        private const val MAX_BODY_CHARS = 300

        fun parse(content: ByteArray): PushPayload {
            val message = runCatching {
                Json.parseToJsonElement(content.decodeToString()).jsonObject
            }.getOrNull() ?: return Wake
            fun text(key: String) = runCatching { message[key]?.jsonPrimitive?.contentOrNull }
                .getOrNull()?.trim().orEmpty()
            if (text("type") != "test") return Wake
            return Test(
                title = text("title").ifEmpty { DEFAULT_TEST_TITLE }.take(MAX_TITLE_CHARS),
                body = text("body").take(MAX_BODY_CHARS),
            )
        }
    }
}

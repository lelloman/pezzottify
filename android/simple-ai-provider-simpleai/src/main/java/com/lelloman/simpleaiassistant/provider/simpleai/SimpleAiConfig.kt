package com.lelloman.simpleaiassistant.provider.simpleai

/**
 * Configuration for the SimpleAI LLM provider.
 *
 * SimpleAI communicates with the SimpleAI Android app via AIDL.
 * The auth token is passed to the SimpleAI app which handles the cloud LLM calls.
 *
 * @param authToken The authentication token for cloud AI calls
 */
data class SimpleAiConfig(
    val authToken: String
)

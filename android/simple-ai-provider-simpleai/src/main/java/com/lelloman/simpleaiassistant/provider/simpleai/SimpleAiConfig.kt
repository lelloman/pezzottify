package com.lelloman.simpleaiassistant.provider.simpleai

/**
 * Configuration for the SimpleAI LLM provider.
 *
 * SimpleAI communicates with the SimpleAI Android app via AIDL.
 * The auth token provider is called for each request to ensure fresh tokens.
 *
 * @param authTokenProvider Function that returns the current authentication token for cloud AI calls
 */
class SimpleAiConfig(
    val authTokenProvider: () -> String
)

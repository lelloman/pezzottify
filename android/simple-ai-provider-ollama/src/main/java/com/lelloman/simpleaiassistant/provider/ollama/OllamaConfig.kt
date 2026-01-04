package com.lelloman.simpleaiassistant.provider.ollama

/**
 * Configuration for the Ollama LLM provider.
 *
 * @param baseUrl The base URL of the Ollama server (e.g., "http://localhost:11434")
 * @param model The model to use for chat (e.g., "llama3.2", "mistral")
 * @param timeoutMs Connection and read timeout in milliseconds
 */
data class OllamaConfig(
    val baseUrl: String,
    val model: String,
    val timeoutMs: Long = 60_000L
)

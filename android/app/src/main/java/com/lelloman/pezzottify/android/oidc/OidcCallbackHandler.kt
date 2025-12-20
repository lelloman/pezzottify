package com.lelloman.pezzottify.android.oidc

import android.content.Intent
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles OIDC callbacks by receiving intents from MainActivity
 * and making them available to the login flow.
 */
@Singleton
class OidcCallbackHandler @Inject constructor() {

    private val _callbacks = MutableSharedFlow<Intent>(replay = 1)
    val callbacks: SharedFlow<Intent> = _callbacks.asSharedFlow()

    // Track the last processed callback URI to prevent duplicate processing
    private var lastProcessedCallbackUri: String? = null

    /**
     * Called by MainActivity when an OIDC callback intent is received.
     */
    fun handleCallback(intent: Intent) {
        _callbacks.tryEmit(intent)
    }

    /**
     * Check if a callback has already been processed.
     * Call this before processing to avoid duplicates.
     */
    fun isAlreadyProcessed(intent: Intent): Boolean {
        val uri = intent.data?.toString() ?: return false
        return uri == lastProcessedCallbackUri
    }

    /**
     * Mark a callback as processed.
     * Call this after successfully starting to process a callback.
     */
    fun markAsProcessed(intent: Intent) {
        lastProcessedCallbackUri = intent.data?.toString()
    }

    /**
     * Clear the processed state (e.g., when starting a new auth flow).
     */
    fun clearProcessedState() {
        lastProcessedCallbackUri = null
    }

    /**
     * Check if an intent is an OIDC callback.
     */
    fun isOidcCallback(intent: Intent): Boolean {
        val uri = intent.data ?: return false
        return uri.scheme == OIDC_REDIRECT_SCHEME &&
                uri.host == OIDC_REDIRECT_HOST &&
                uri.path == OIDC_REDIRECT_PATH
    }

    companion object {
        const val OIDC_REDIRECT_SCHEME = "com.lelloman.pezzottify.android"
        const val OIDC_REDIRECT_HOST = "oauth"
        const val OIDC_REDIRECT_PATH = "/callback"
    }
}

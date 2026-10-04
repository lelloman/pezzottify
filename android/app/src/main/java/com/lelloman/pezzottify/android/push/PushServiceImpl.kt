package com.lelloman.pezzottify.android.push

import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

/**
 * Receives UnifiedPush events from the distributor. Messages carry no user content: they
 * wake the app to run its sync catch-up, except admin test notifications, which are shown
 * (docs/unifiedpush.md).
 */
class PushServiceImpl : PushService() {

    private val registration by lazy { applicationContext.pushRegistration() }

    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        registration.onNewEndpoint(endpoint.url, endpoint.pubKeySet?.pubKey, endpoint.pubKeySet?.auth)
    }

    override fun onMessage(message: PushMessage, instance: String) {
        registration.onMessage(message.content)
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        registration.onRegistrationFailed(reason.name)
    }

    override fun onUnregistered(instance: String) {
        registration.onUnregistered()
    }
}

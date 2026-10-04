package com.lelloman.pezzottify.android.push

import android.content.Context
import androidx.core.content.edit
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject

/** Local push registration state. */
interface PushPreferences {
    var enabled: Boolean

    /** Endpoint and Web Push keys most recently issued by the distributor. */
    var endpoint: String?
    var p256dh: String?
    var auth: String?

    /** Distributor and VAPID key the current registration was made with. */
    var registeredDistributor: String?
    var vapidKey: String?

    /** Endpoint the server has confirmed; differs from [endpoint] until the PUT succeeds. */
    var serverEndpoint: String?

    fun clearRegistration() {
        endpoint = null
        p256dh = null
        auth = null
        registeredDistributor = null
        vapidKey = null
        serverEndpoint = null
    }
}

class SharedPreferencesPushPreferences @Inject constructor(
    @ApplicationContext context: Context,
) : PushPreferences {

    private val prefs = context.getSharedPreferences("unified_push", Context.MODE_PRIVATE)

    override var enabled: Boolean
        get() = prefs.getBoolean(KEY_ENABLED, true)
        set(value) = prefs.edit { putBoolean(KEY_ENABLED, value) }

    override var endpoint by string(KEY_ENDPOINT)
    override var p256dh by string(KEY_P256DH)
    override var auth by string(KEY_AUTH)
    override var registeredDistributor by string(KEY_DISTRIBUTOR)
    override var vapidKey by string(KEY_VAPID)
    override var serverEndpoint by string(KEY_SERVER_ENDPOINT)

    private fun string(key: String) = object : kotlin.properties.ReadWriteProperty<Any, String?> {
        override fun getValue(thisRef: Any, property: kotlin.reflect.KProperty<*>): String? =
            prefs.getString(key, null)

        override fun setValue(thisRef: Any, property: kotlin.reflect.KProperty<*>, value: String?) =
            prefs.edit { if (value == null) remove(key) else putString(key, value) }
    }

    private companion object {
        const val KEY_ENABLED = "enabled"
        const val KEY_ENDPOINT = "endpoint"
        const val KEY_P256DH = "p256dh"
        const val KEY_AUTH = "auth"
        const val KEY_DISTRIBUTOR = "registered_distributor"
        const val KEY_VAPID = "vapid_key"
        const val KEY_SERVER_ENDPOINT = "server_endpoint"
    }
}

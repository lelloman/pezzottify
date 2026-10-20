package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.domain.maintenance.MaintenanceEventBus
import okhttp3.Interceptor
import okhttp3.Response

/**
 * Signals maintenance mode whenever the server rejects a request with the
 * dedicated maintenance response. The UI then shows the blocking screen.
 */
class MaintenanceInterceptor(
    private val maintenanceEventBus: MaintenanceEventBus,
) : Interceptor {

    override fun intercept(chain: Interceptor.Chain): Response {
        val response = chain.proceed(chain.request())
        if (response.code == 503 && response.header("X-Maintenance-Mode") == "1") {
            maintenanceEventBus.emit()
        }
        return response
    }
}

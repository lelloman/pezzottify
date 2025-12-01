package com.lelloman.pezzottify.android.debuginterface

import android.util.Log
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import java.net.Inet4Address
import java.net.NetworkInterface
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DebugServerInitializer @Inject constructor(
    private val debugHttpServer: DebugHttpServer
) : AppInitializer {

    companion object {
        private const val TAG = "DebugServer"
    }

    override fun initialize() {
        try {
            debugHttpServer.start()
            val localIp = getLocalIpAddress() ?: "localhost"
            Log.i(TAG, "Debug HTTP server started on port ${DebugHttpServer.DEFAULT_PORT}")
            Log.i(TAG, "From device browser: http://$localIp:${DebugHttpServer.DEFAULT_PORT}")
            Log.i(TAG, "From host: run 'adb forward tcp:${DebugHttpServer.DEFAULT_PORT} tcp:${DebugHttpServer.DEFAULT_PORT}' then open http://localhost:8080")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start debug HTTP server", e)
        }
    }

    private fun getLocalIpAddress(): String? {
        try {
            val interfaces = NetworkInterface.getNetworkInterfaces()
            while (interfaces.hasMoreElements()) {
                val networkInterface = interfaces.nextElement()
                val addresses = networkInterface.inetAddresses
                while (addresses.hasMoreElements()) {
                    val address = addresses.nextElement()
                    if (!address.isLoopbackAddress && address is Inet4Address) {
                        return address.hostAddress
                    }
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Could not determine local IP address", e)
        }
        return null
    }
}

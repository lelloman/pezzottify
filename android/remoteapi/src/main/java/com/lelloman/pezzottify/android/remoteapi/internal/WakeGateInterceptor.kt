package com.lelloman.pezzottify.android.remoteapi.internal

import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.util.concurrent.CopyOnWriteArraySet
import okhttp3.HttpUrl
import okhttp3.Interceptor
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response

/** Waits for readiness before sending the original request exactly once. */
internal class WakeGateInterceptor(private val origin: HttpUrl) : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val request = chain.request()
        if (request.header("X-Wake-Mode") != "passive" && request.url.queryParameter("wake") != "passive"
            && request.url.host == origin.host && request.url.port == origin.port
            && request.url.scheme == origin.scheme) {
            awaitReady(origin)
        }
        return chain.proceed(request)
    }

    companion object {
        private val readyListeners = CopyOnWriteArraySet<() -> Unit>()
        internal fun onServerReady(listener: () -> Unit): () -> Unit {
            readyListeners.add(listener)
            return { readyListeners.remove(listener); Unit }
        }
        private data class State(var readyUntil: Long = 0, var legacyUntil: Long = 0)
        private val states = ConcurrentHashMap<String, State>()
        // Deliberately separate from the application client to avoid recursion.
        private val probe = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(125, TimeUnit.SECONDS)
            .callTimeout(125, TimeUnit.SECONDS)
            .build()

        private fun endpoint(origin: HttpUrl, path: String) = origin.newBuilder()
            .encodedPath(path).query(null).fragment(null).build()

        private fun state(response: Response): String? = try {
            Json.parseToJsonElement(response.body?.string() ?: "").jsonObject["state"]?.jsonPrimitive?.content
        } catch (_: Exception) { null }

        private fun awaitReady(origin: HttpUrl) {
            val key = "${origin.scheme}://${origin.host}:${origin.port}"
            val admission = states.computeIfAbsent(key) { State() }
            synchronized(admission) {
                val now = System.nanoTime()
                if (now < admission.readyUntil || now < admission.legacyUntil) return
                val status = Request.Builder().url(endpoint(origin, "/_power/status")).build()
                val serverState = probe.newCall(status).execute().use { response ->
                    if (response.code == 404 || (response.isSuccessful && response.header("Content-Type")?.contains("application/json") != true)) {
                        admission.legacyUntil = now + TimeUnit.MINUTES.toNanos(1)
                        return
                    }
                    if (!response.isSuccessful) throw IOException("Server availability check failed")
                    state(response)
                }
                when (serverState) {
                    "ready" -> Unit
                    "sleeping" -> {
                        val wake = Request.Builder().url(endpoint(origin, "/_power/wake"))
                            .post("".toRequestBody(null)).build()
                        probe.newCall(wake).execute().use { response ->
                            if (!response.isSuccessful || state(response) != "ready") {
                                throw IOException("Server is still waking; try again shortly")
                            }
                        }
                    }
                    else -> throw IOException("Invalid server availability response")
                }
                admission.readyUntil = System.nanoTime() + TimeUnit.SECONDS.toNanos(2)
                readyListeners.forEach { it() }
            }
        }
    }
}

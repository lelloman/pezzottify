package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.logger.Logger
import okhttp3.Interceptor
import okhttp3.Response
import okio.Buffer
import java.nio.charset.Charset
import java.util.concurrent.TimeUnit

internal class HttpLoggingInterceptor(
    private val logger: Logger,
) : Interceptor {

    override fun intercept(chain: Interceptor.Chain): Response {
        val request = chain.request()

        val requestLog = buildString {
            append("--> ${request.method} ${request.url}")
            request.body?.let { body ->
                val buffer = Buffer()
                body.writeTo(buffer)
                val bodyString = buffer.readString(Charset.forName("UTF-8"))
                if (bodyString.isNotEmpty()) {
                    append(" | body: ${bodyString.truncate(500)}")
                }
            }
        }
        logger.debug(requestLog)

        val startNs = System.nanoTime()
        val response: Response
        try {
            response = chain.proceed(request)
        } catch (e: Exception) {
            logger.error("<-- ${request.method} ${request.url} FAILED: ${e.message}")
            throw e
        }

        val durationMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - startNs)

        val responseLog = buildString {
            append("<-- ${response.code} ${request.url} (${durationMs}ms)")
            if (!response.isSuccessful) {
                response.peekBodySafe()?.let { bodyPreview ->
                    append(" | body: ${bodyPreview.truncate(500)}")
                }
            }
        }

        if (response.isSuccessful) {
            logger.debug(responseLog)
        } else {
            logger.warn(responseLog)
        }

        return response
    }

    private fun Response.peekBodySafe(): String? {
        return try {
            body?.source()?.let { source ->
                source.request(Long.MAX_VALUE)
                source.buffer.clone().readString(Charset.forName("UTF-8"))
            }
        } catch (e: Exception) {
            null
        }
    }

    private fun String.truncate(maxLength: Int): String {
        return if (length <= maxLength) this else "${take(maxLength)}..."
    }
}

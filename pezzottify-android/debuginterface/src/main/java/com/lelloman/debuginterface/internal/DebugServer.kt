package com.lelloman.debuginterface.internal

import com.google.gson.GsonBuilder
import com.lelloman.debuginterface.DebugOperation
import fi.iki.elonen.NanoHTTPD
import java.io.PrintWriter
import java.io.StringWriter

internal class DebugServer(
    private val operations: List<DebugOperation>,
    gsonBuilder: GsonBuilder = GsonBuilder(),
) : NanoHTTPD(8889) {

    private val gson = gsonBuilder
        .setPrettyPrinting()
        .create()

    private val postInterceptors: List<PostInterceptor<*>> = operations
        .filterIsInstance<DebugOperation.SimpleAction<*>>()
        .map { PostInterceptor(it.getKey(), it.action) }

    override fun serve(session: IHTTPSession?): Response {
        val path = session?.uri ?: return newFixedLengthResponse("")
        if (session.method == Method.POST) {
            return handlePost(session)
        }
        if (session.method == Method.GET && path == "/") {
            return home(operations)
        }
        return notFound()
    }

    private fun handlePost(session: IHTTPSession): Response {
        val interceptor =
            postInterceptors.firstOrNull { it.matches(session.uri) } ?: return notFound()
        return try {
            val resultString = when (val result = interceptor.action()) {
                is Unit -> "OK"
                is String -> result
                is Int, Long, Boolean -> result.toString()
                else -> gson.toJson(result)
            }
            newFixedLengthResponse(resultString)
        } catch (e: Throwable) {
            val stringWriter = StringWriter()
            val printWriter = PrintWriter(stringWriter)
            e.printStackTrace(printWriter)
            newFixedLengthResponse(stringWriter.toString())
        }
    }

    private fun notFound(): Response {
        return newFixedLengthResponse(Response.Status.NOT_FOUND, "text/plain", "NOT FOUND")
    }

    private class PostInterceptor<T>(private val key: String, val action: () -> T) {
        fun matches(path: String) = path == "/$key"
    }
}
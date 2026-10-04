package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Test
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class StreamingSearchCancellationTest {

    @Test
    fun `cancelling the collector closes a stream that sends no events`() = runBlocking {
        val requested = CountDownLatch(1)
        val connectionClosed = CountDownLatch(1)
        val server = ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))
        Thread {
            server.accept().use { socket ->
                val input = socket.getInputStream().bufferedReader()
                while (input.readLine()?.isNotEmpty() == true) Unit
                val output = socket.getOutputStream()
                output.write("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n".toByteArray())
                requested.countDown()
                // Keep-alive comments only: the client reads lines but never emits, so only
                // cancelling the HTTP call can end the collection.
                try {
                    while (true) {
                        output.write(": keep-alive\n\n".toByteArray())
                        output.flush()
                        Thread.sleep(50)
                    }
                } catch (_: IOException) {
                    connectionClosed.countDown()
                }
            }
        }.apply { isDaemon = true }.start()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val api = RemoteApiClientImpl(
                object : RemoteApiClient.HostUrlProvider {
                    override val hostUrl = MutableStateFlow("http://127.0.0.1:${server.localPort}")
                },
                OkHttpClientFactory(),
                object : RemoteApiCredentialsProvider { override val authToken = "token" },
                scope,
            )
            val received = mutableListOf<SearchSection>()
            val collection = launch(Dispatchers.Default) {
                api.streamingSearch("jazz", excludeUnavailable = false).collect { received += it }
            }
            assertThat(requested.await(5, TimeUnit.SECONDS)).isTrue()

            withTimeout(3_000) { collection.cancelAndJoin() }

            assertThat(connectionClosed.await(3, TimeUnit.SECONDS)).isTrue()
            assertThat(received).isEmpty()
        } finally {
            scope.cancel()
            server.close()
        }
    }
}

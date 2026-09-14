package com.lelloman.pezzottify.android.ui.screen.main.assistant

import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

@Singleton
class AssistantConfirmation @Inject constructor() {
    data class Request(val id: String, val action: String, val details: String)
    private val mutex = Mutex()
    private val mutableRequest = MutableStateFlow<Request?>(null)
    val request = mutableRequest.asStateFlow()
    private data class Pending(val request: Request, val answer: CompletableDeferred<Boolean>)
    @Volatile private var pending: Pending? = null

    suspend fun confirm(action: String, details: String): Boolean = mutex.withLock {
        val deferred = CompletableDeferred<Boolean>()
        val request = Request(UUID.randomUUID().toString(), action, details)
        pending = Pending(request, deferred)
        mutableRequest.value = request
        try {
            withTimeoutOrNull(60_000) { deferred.await() } ?: false
        } finally {
            mutableRequest.value = null
            pending = null
        }
    }

    fun respond(id: String, approved: Boolean) {
        val active = pending
        if (active?.request?.id == id) active.answer.complete(approved)
    }
}

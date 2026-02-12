package com.lelloman.pezzottify.android.player

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
internal class PlayerServiceEventsEmitter @Inject constructor(
    private val applicationScope: CoroutineScope,
) {

    private val mutableEvents = MutableSharedFlow<Event>()
    val events = mutableEvents.asSharedFlow()

    fun shutdown() {
        applicationScope.launch {
            mutableEvents.emit(Event.Shutdown)
        }
    }

    enum class Event {
        Shutdown
    }
}